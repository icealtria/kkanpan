package main

import (
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"
)

var (
	cachedData []StockData
	cacheMutex sync.RWMutex
	lastFetch  int64
	priceHist  = make(map[string][]float64)
	histMutex  sync.Mutex

	httpClient = &http.Client{
		Timeout: 8 * time.Second,
		Transport: &http.Transport{
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
		},
	}
	yahooClient = &http.Client{
		Timeout: 8 * time.Second,
		Transport: &http.Transport{
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
		},
	}
)

func initClients() {
	if appConfig.Proxy != "" {
		u, err := url.Parse(appConfig.Proxy)
		if err == nil {
			httpClient.Transport = &http.Transport{
				TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
				Proxy:           http.ProxyURL(u),
			}
			yahooClient.Transport = &http.Transport{
				TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
				Proxy:           http.ProxyURL(u),
			}
		}
	}
	if appConfig.CacheTTL != 0 {
		cacheTTL = appConfig.CacheTTL
	}
}

var cacheTTL int64 = 55
var qtRe = regexp.MustCompile(`v_(\w+)="([^"]*)"`)

// neededStocks 按当前 Tab 只返回需要拉取的配置 (AUTO 未命中则仅常驻组)
func neededStocks() []StockConfig {
	all := loadStocks()
	mode := GetViewMode()
	eff, isAuto := GetEffectiveGroup(mode)
	if eff == "" {
		if !isAuto || len(appConfig.PinnedGroups) == 0 {
			return nil
		}
		// AUTO 空档期仍显示常驻组
		want := make(map[string]bool, len(appConfig.PinnedGroups))
		for _, pg := range appConfig.PinnedGroups {
			want[pg] = true
		}
		var out []StockConfig
		for _, c := range all {
			if want[c.Group] {
				out = append(out, c)
			}
		}
		return out
	}
	if eff == "ALL" {
		return all
	}
	want := map[string]bool{eff: true}
	if isAuto {
		for _, pg := range appConfig.PinnedGroups {
			if pg != eff {
				want[pg] = true
			}
		}
	}
	var out []StockConfig
	for _, c := range all {
		if want[c.Group] {
			out = append(out, c)
		}
	}
	return out
}

func fetchQT(codes []string) map[string][]string {
	out := make(map[string][]string)
	if len(codes) == 0 {
		return out
	}
	urlStr := "https://qt.gtimg.cn/q=" + strings.Join(codes, ",")
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("Referer", "https://gu.qq.com/")
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := httpClient.Do(req)
	if err != nil {
		return out
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	for _, m := range qtRe.FindAllSubmatch(body, -1) {
		code := string(m[1])
		raw := string(m[2])
		var vals []string
		if strings.Contains(raw, "~") {
			vals = strings.Split(raw, "~")
		} else {
			vals = strings.Split(raw, ",")
		}
		out[code] = vals
	}
	return out
}

func fetchYahoo(code string) ([]float64, float64, float64) {
	urlStr := fmt.Sprintf("https://query1.finance.yahoo.com/v8/finance/chart/%s?interval=1m&range=1d", url.QueryEscape(code))
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := yahooClient.Do(req)
	if err != nil {
		return nil, 0, 0
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
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

func fetchMinute(code string) []float64 {
	urlStr := "https://web.ifzq.gtimg.cn/appstock/app/minute/query?code=" + code
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := httpClient.Do(req)
	if err != nil {
		return nil
	}
	defer resp.Body.Close()
	var j map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&j); err != nil {
		return nil
	}
	dataMap, _ := j["data"].(map[string]interface{})
	item, _ := dataMap[code].(map[string]interface{})
	d, _ := item["data"].(map[string]interface{})
	rows, _ := d["data"].([]interface{})
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
	if len(prices) < 2 {
		return nil
	}
	return prices
}

func fetchUsMinute(code string) []float64 {
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
	configs := neededStocks()
	// 日志: 按需拉取前后对比 (全量 ~14 -> 单 Tab 3~6)
	if len(configs) != len(loadStocks()) {
		eff, _ := GetEffectiveGroup(GetViewMode())
		log.Printf("[fetch] View %s: fetching %d/%d stocks", eff, len(configs), len(loadStocks()))
	}
	var tencentCodes []string
	for _, c := range configs {
		if c.Source == "tencent" {
			tencentCodes = append(tencentCodes, c.Code)
		}
	}
	qt := fetchQT(tencentCodes)

	chartMap := make(map[string]chartResult)
	var wg sync.WaitGroup
	var mu sync.Mutex
	for _, c := range configs {
		wg.Add(1)
		go func(cfg StockConfig) {
			defer wg.Done()
			var res chartResult
			switch cfg.Source {
			case "tencent":
				if strings.HasPrefix(cfg.Code, "sh") || strings.HasPrefix(cfg.Code, "sz") {
					res.prices = fetchMinute(cfg.Code)
				} else if strings.HasPrefix(cfg.Code, "us") {
					res.prices = fetchUsMinute(cfg.Code)
				}
			case "yahoo":
				res.prices, res.yPrice, res.yPrev = fetchYahoo(cfg.Code)
			}
			mu.Lock()
			chartMap[cfg.Code] = res
			mu.Unlock()
		}(c)
	}
	wg.Wait()

	var items []StockData
	for _, c := range configs {
		code := c.Code
		cRes := chartMap[code]
		var price, change, pct, prev float64
		var prices []float64

		switch c.Source {
		case "tencent":
			vals := qt[code]
			if len(vals) == 0 {
				clean := strings.ReplaceAll(strings.ReplaceAll(code, "sh", ""), "sz", "")
				vals = qt[clean]
			}
			price, change, pct, prev = parseQT(code, vals)
			if code == "^VIX" {
				price, change, pct, prev = 0, 0, 0, 0
			}
			prices = cRes.prices
		case "yahoo":
			price, prev = cRes.yPrice, cRes.yPrev
			if prev != 0 {
				change = price - prev
				pct = (change / prev) * 100
			}
			prices = cRes.prices
			if code == "^VIX" && price == 0 {
				price, change, pct, prev = 0, 0, 0, 0
			}
		}

		isChart := false
		if c.Source == "tencent" {
			isChart = strings.HasPrefix(code, "sh") || strings.HasPrefix(code, "sz") || strings.HasPrefix(code, "us")
		} else {
			isChart = len(prices) >= 2 || price > 0
		}
		if isChart {
			if len(prices) < 2 {
				histMutex.Lock()
				h := priceHist[code]
				if price > 0 {
					h = append(h, price)
					if len(h) > tradingMinutes(code) && tradingMinutes(code) > 0 {
						h = h[len(h)-tradingMinutes(code):]
					} else if len(h) > 1440 {
						h = h[len(h)-1440:]
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
	lastFetch = time.Now().Unix()
	cacheMutex.Unlock()
	return items
}

func getData() []StockData {
	needed := neededStocks()
	if len(needed) == 0 {
		return nil // AUTO 未命中 -> 空
	}
	needSet := make(map[string]bool, len(needed))
	for _, c := range needed {
		needSet[c.Code] = true
	}
	cacheMutex.RLock()
	now := time.Now().Unix()
	ttl := cacheTTL
	if appConfig.CacheTTL != 0 {
		ttl = appConfig.CacheTTL
	}
	if now-lastFetch <= ttl && len(cachedData) > 0 {
		// 检查缓存是否覆盖当前 Tab 所需代码, 否则穿透刷新 (切 Tab 后缓存未命中)
		cachedCodes := make(map[string]bool, len(cachedData))
		for _, d := range cachedData {
			cachedCodes[d.Code] = true
		}
		hit := true
		for code := range needSet {
			if !cachedCodes[code] {
				hit = false
				break
			}
		}
		if hit {
			defer cacheMutex.RUnlock()
			return cachedData
		}
	}
	cacheMutex.RUnlock()
	return refreshData()
}
