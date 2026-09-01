package main

import (
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"
)

var (
	cachedData  []StockData
	cacheMutex  sync.RWMutex
	lastFetchTs int64
	cacheTTL    int64 = 55
	priceHist         = make(map[string][]float64)
	histMutex   sync.Mutex

	httpClient = &http.Client{
		Timeout: 8 * time.Second,
		Transport: &http.Transport{
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
		},
	}
)

func fetchQT(codes []string) map[string][]string {
	out := make(map[string][]string)
	if len(codes) == 0 {
		return out
	}
	urlStr := "https://qt.gtimg.cn/q=" + strings.Join(codes, ",")
	req, err := http.NewRequest("GET", urlStr, nil)
	if err != nil {
		return out
	}
	req.Header.Set("Referer", "https://gu.qq.com/")
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := httpClient.Do(req)
	if err != nil {
		return out
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return out
	}
	re := regexp.MustCompile(`v_(\w+)="([^"]*)"`)
	matches := re.FindAllSubmatch(body, -1)
	for _, m := range matches {
		code := string(m[1])
		rawVal := string(m[2])
		var vals []string
		if strings.Contains(rawVal, "~") {
			vals = strings.Split(rawVal, "~")
		} else {
			vals = strings.Split(rawVal, ",")
		}
		out[code] = vals
	}
	return out
}

func fetchYahoo(code string) ([]float64, float64, float64) {
	yahooSymbol := codeToYahoo(code)
	urlStr := fmt.Sprintf("https://query1.finance.yahoo.com/v8/finance/chart/%s?interval=1m&range=1d", url.QueryEscape(yahooSymbol))
	req, err := http.NewRequest("GET", urlStr, nil)
	if err != nil {
		return nil, 0, 0
	}
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := httpClient.Do(req)
	if err != nil {
		return nil, 0, 0
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, 0, 0
	}
	var parsed struct {
		Chart struct {
			Result []struct {
				Meta struct {
					RegularMarketPrice float64 `json:"regularMarketPrice"`
					PreviousClose      float64 `json:"previousClose"`
					ChartPreviousClose float64 `json:"chartPreviousClose"`
				} `json:"meta"`
				Indicators struct {
					Quote []struct {
						Close []*float64 `json:"close"`
					} `json:"quote"`
				} `json:"indicators"`
			} `json:"result"`
		} `json:"chart"`
	}
	if err := json.Unmarshal(body, &parsed); err != nil || len(parsed.Chart.Result) == 0 {
		return nil, 0, 0
	}
	res := parsed.Chart.Result[0]
	var prices []float64
	if len(res.Indicators.Quote) > 0 {
		for _, v := range res.Indicators.Quote[0].Close {
			if v != nil {
				prices = append(prices, *v)
			}
		}
	}
	price := res.Meta.RegularMarketPrice
	if price == 0 && len(prices) > 0 {
		price = prices[len(prices)-1]
	}
	prev := res.Meta.PreviousClose
	if prev == 0 {
		prev = res.Meta.ChartPreviousClose
	}
	return prices, price, prev
}

func resolveEastmoneyQuoteID(sym string) string {
	// ponytail: 动态解析不同平台代码差异，东财 QuoteID 如 105.AAPL
	urlStr := fmt.Sprintf("https://searchapi.eastmoney.com/api/suggest/get?input=%s&type=14&token=D43BF722C8E33BDC906FB84D85E326E8", url.QueryEscape(sym))
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("Referer", "https://quote.eastmoney.com/")
	resp, err := httpClient.Do(req)
	if err != nil {
		return ""
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	var r struct {
		QuotationCodeTable struct {
			Data []struct {
				QuoteID string `json:"QuoteID"`
				Code    string `json:"Code"`
			} `json:"Data"`
		} `json:"QuotationCodeTable"`
	}
	if err := json.Unmarshal(body, &r); err != nil || len(r.QuotationCodeTable.Data) == 0 {
		return ""
	}
	// 优先精确匹配 Code
	for _, d := range r.QuotationCodeTable.Data {
		if strings.EqualFold(d.Code, sym) {
			return d.QuoteID
		}
	}
	return r.QuotationCodeTable.Data[0].QuoteID
}

func fetchEastmoneyPrices(code string) []float64 {
	// 仅对美股，期货走历史兜底避免代码错配
	if strings.HasPrefix(code, "hf_") {
		return nil // ponytail: 期货跨平台代码差异大（GC=F vs 105.AU），暂用 priceHist
	}
	sym := codeToYahoo(code)
	sym = strings.TrimPrefix(sym, "^")
	sym = strings.TrimSuffix(sym, "=F")
	if idx := strings.Index(sym, "."); idx >= 0 {
		sym = sym[:idx]
	}
	if sym == "" {
		return nil
	}
	quoteID := resolveEastmoneyQuoteID(sym)
	var secids []string
	var syms []string
	if quoteID != "" && strings.Contains(quoteID, ".") {
		parts := strings.SplitN(quoteID, ".", 2)
		secids = []string{parts[0]}
		syms = []string{parts[1]}
	} else {
		// 回退硬编码，兼容旧逻辑
		secids = []string{"105", "100", "106"}
		syms = []string{sym, sym, sym}
	}
	today := time.Now().Format("20060102")
	tomorrow := time.Now().Add(24 * time.Hour).Format("20060102")
	yesterday := time.Now().Add(-24 * time.Hour).Format("20060102")
	urls := []string{
		fmt.Sprintf("https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=%%s.%%s&klt=1&fqt=1&lmt=500"),
		fmt.Sprintf("https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=%%s.%%s&klt=1&fqt=1&beg=%s&end=%s", today, tomorrow),
		fmt.Sprintf("https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=%%s.%%s&klt=1&fqt=1&beg=%s&end=%s", yesterday, today),
	}
	for i, secid := range secids {
		s := syms[0]
		if len(syms) > i {
			s = syms[i]
		}
		for _, tmpl := range urls {
			urlStr := fmt.Sprintf(tmpl, secid, url.QueryEscape(s))
			req, _ := http.NewRequest("GET", urlStr, nil)
			req.Header.Set("Referer", "https://quote.eastmoney.com/")
			resp, err := httpClient.Do(req)
			if err != nil {
				continue
			}
			body, _ := io.ReadAll(resp.Body)
			resp.Body.Close()
			var j struct {
				Data struct {
					Klines []string `json:"klines"`
				} `json:"data"`
			}
			if err := json.Unmarshal(body, &j); err != nil || len(j.Data.Klines) < 2 {
				continue
			}
			var prices []float64
			for _, k := range j.Data.Klines {
				parts := strings.Split(k, ",")
				if len(parts) < 3 {
					continue
				}
				if p, err := strconv.ParseFloat(parts[2], 64); err == nil && p > 0 {
					prices = append(prices, p)
				}
			}
			if len(prices) > 2 {
				return prices
			}
		}
	}
	return nil
}

func fetchMinute(code string) []float64 {
	if strings.HasPrefix(code, "sh") || strings.HasPrefix(code, "sz") {
		urlStr := "https://web.ifzq.gtimg.cn/appstock/app/minute/query?code=" + code
		req, _ := http.NewRequest("GET", urlStr, nil)
		resp, err := httpClient.Do(req)
		if err == nil {
			defer resp.Body.Close()
			var j map[string]interface{}
			if err := json.NewDecoder(resp.Body).Decode(&j); err == nil {
				if dataMap, ok := j["data"].(map[string]interface{}); ok {
					if item, ok := dataMap[code].(map[string]interface{}); ok {
						if d, ok := item["data"].(map[string]interface{}); ok {
							if rows, ok := d["data"].([]interface{}); ok {
								var prices []float64
								for _, row := range rows {
									if str, ok := row.(string); ok {
										parts := strings.Fields(str)
										if len(parts) >= 2 {
											if p, err := strconv.ParseFloat(parts[1], 64); err == nil {
												prices = append(prices, p)
											}
										}
									}
								}
								if len(prices) > 2 {
									return prices
								}
							}
						}
					}
				}
			}
		}
	}
	prices, _, _ := fetchYahoo(code)
	return prices
}

func fetchUsMinute(code string) []float64 {
	// gu.qq.com/usAAPL.OQ/gg 同源：web.ifzq.gtimg.cn/appstock/app/UsMinute/query?code=usAAPL
	urlStr := "https://web.ifzq.gtimg.cn/appstock/app/UsMinute/query?code=" + url.QueryEscape(code)
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("Referer", "https://gu.qq.com/")
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := httpClient.Do(req)
	if err != nil {
		return nil
	}
	defer resp.Body.Close()
	var j struct {
		Data map[string]struct {
			Data struct {
				Data []string `json:"data"`
			} `json:"data"`
		} `json:"data"`
		Code int `json:"code"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&j); err != nil || j.Code != 0 {
		return nil
	}
	item, ok := j.Data[code]
	if !ok || len(item.Data.Data) < 2 {
		return nil
	}
	var prices []float64
	for _, row := range item.Data.Data {
		parts := strings.Fields(row)
		if len(parts) >= 2 {
			if p, err := strconv.ParseFloat(parts[1], 64); err == nil && p > 0 {
				prices = append(prices, p)
			}
		}
	}
	if len(prices) < 2 {
		return nil
	}
	return prices
}

func parseQT(code string, vals []string) (price, change, pct, prev float64) {
	defer func() {
		if r := recover(); r != nil {
			price, change, pct, prev = 0, 0, 0, 0
		}
	}()
	if strings.HasPrefix(code, "hf_") {
		if len(vals) > 0 {
			price, _ = strconv.ParseFloat(vals[0], 64)
		}
		if len(vals) > 1 {
			change, _ = strconv.ParseFloat(vals[1], 64)
		}
		prev = price - change
		if prev != 0 {
			pct = (change / prev) * 100
		}
	} else {
		if len(vals) > 3 {
			price, _ = strconv.ParseFloat(vals[3], 64)
		}
		if len(vals) > 4 {
			prev, _ = strconv.ParseFloat(vals[4], 64)
		}
		if prev == 0 {
			prev = price
		}
		if len(vals) > 32 {
			pct, _ = strconv.ParseFloat(vals[32], 64)
		}
		if len(vals) > 31 {
			change, _ = strconv.ParseFloat(vals[31], 64)
		} else {
			change = price - prev
		}
	}
	return
}

type chartResult struct {
	prices []float64
	yPrice float64
	yPrev  float64
}

func refreshData() []StockData {
	configs := loadConfig()
	var codes []string
	for _, c := range configs {
		codes = append(codes, c.Code)
	}
	qt := fetchQT(codes)
	chartMap := make(map[string]chartResult)
	var wg sync.WaitGroup
	var mu sync.Mutex
	for _, c := range configs {
		wg.Add(1)
		go func(code string) {
			defer wg.Done()
			var res chartResult
			if strings.HasPrefix(code, "sh") || strings.HasPrefix(code, "sz") {
				res.prices = fetchMinute(code)
			} else if strings.HasPrefix(code, "us") {
				res.prices = fetchUsMinute(code) // ponytail: 腾讯 UsMinute 直连稳定，期货仍无图
			} else {
				res.prices = nil
			}
			mu.Lock()
			chartMap[code] = res
			mu.Unlock()
		}(c.Code)
	}
	wg.Wait()

	var items []StockData
	for _, c := range configs {
		code := c.Code
		vals := qt[code]
		if len(vals) == 0 {
			clean := strings.ReplaceAll(strings.ReplaceAll(code, "sh", ""), "sz", "")
			vals = qt[clean]
		}
		price, change, pct, prev := parseQT(code, vals)
		cRes := chartMap[code]
		if cRes.yPrice > 0 && cRes.yPrev > 0 {
			price = cRes.yPrice
			prev = cRes.yPrev
			change = price - prev
			if prev != 0 {
				pct = (change / prev) * 100
			}
		}
		prices := cRes.prices
		// ponytail: 仅A股/美股维护分时，期货无图不进 priceHist
		isChart := strings.HasPrefix(code, "sh") || strings.HasPrefix(code, "sz") || strings.HasPrefix(code, "us")
		if isChart {
			if len(prices) < 2 {
				histMutex.Lock()
				h := priceHist[code]
				if price > 0 {
					h = append(h, price)
					if len(h) > tradingMinutes(code) {
						if tradingMinutes(code) > 0 {
							h = h[len(h)-tradingMinutes(code):]
						} else if len(h) > 1440 {
							h = h[len(h)-1440:]
						}
					}
					priceHist[code] = h
				}
				if len(h) > 2 {
					prices = append([]float64(nil), h...)
				}
				histMutex.Unlock()
			} else {
				histMutex.Lock()
				h := priceHist[code]
				if price > 0 {
					h = append(h, price)
					if len(h) > tradingMinutes(code) && tradingMinutes(code) > 0 {
						h = h[len(h)-tradingMinutes(code):]
					}
					priceHist[code] = h
				}
				histMutex.Unlock()
			}
		} else {
			prices = nil
		}
		svg := ""
		if len(prices) > 2 {
			svg = svgSparkline(prices, 300, 60, code)
		}
		items = append(items, StockData{
			Code: code, Name: c.Name, Group: c.Group,
			Price: price, Change: change, Pct: pct, Prev: prev,
			Prices: prices, SVG: svg,
		})
	}
	cacheMutex.Lock()
	cachedData = items
	lastFetchTs = time.Now().Unix()
	cacheMutex.Unlock()
	return items
}

func getData() []StockData {
	cacheMutex.RLock()
	now := time.Now().Unix()
	if now-lastFetchTs <= cacheTTL && len(cachedData) > 0 {
		defer cacheMutex.RUnlock()
		return cachedData
	}
	cacheMutex.RUnlock()
	return refreshData()
}
