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
)

func initClients() {
	if appConfig.Proxy != "" {
		u, err := url.Parse(appConfig.Proxy)
		if err == nil {
			httpClient.Transport = &http.Transport{
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

func neededStocks() []StockConfig {
	all := loadStocks()
	mode := GetViewMode()
	eff, isAuto := GetEffectiveGroup(mode)
	if eff == "ALL" {
		return all
	}
	if !isAuto {
		if eff == "" {
			return nil
		}
		var out []StockConfig
		for _, c := range all {
			if c.Group == eff {
				out = append(out, c)
			}
		}
		return out
	}
	matching := GetMatchingAutoGroups()
	if len(matching) == 0 {
		return nil
	}
	want := make(map[string]bool, len(matching))
	for _, g := range matching {
		want[g] = true
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

func fetchYahoo(code string) (prices []float64, timestamps []int64, regStart, regEnd int64, price, prev, chartPrev float64) {
	urlStr := fmt.Sprintf("https://query1.finance.yahoo.com/v8/finance/chart/%s?interval=1m&range=1d", url.QueryEscape(code))
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := httpClient.Do(req)
	if err != nil {
		return
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
					CurrentTradingPeriod struct {
						Regular struct {
							Start int64 `json:"start"`
							End   int64 `json:"end"`
						} `json:"regular"`
					} `json:"currentTradingPeriod"`
				} `json:"meta"`
				Timestamp  []int64 `json:"timestamp"`
				Indicators struct {
					Quote []struct {
						Close []*float64 `json:"close"`
					} `json:"quote"`
				} `json:"indicators"`
			} `json:"result"`
		} `json:"chart"`
	}
	if err := json.Unmarshal(body, &parsed); err != nil || len(parsed.Chart.Result) == 0 {
		return
	}
	res := parsed.Chart.Result[0]
	if len(res.Indicators.Quote) > 0 && len(res.Timestamp) > 0 {
		closes := res.Indicators.Quote[0].Close
		for i, v := range closes {
			if i >= len(res.Timestamp) {
				break
			}
			if v != nil {
				prices = append(prices, *v)
				timestamps = append(timestamps, res.Timestamp[i])
			}
		}
	} else if len(res.Indicators.Quote) > 0 {
		for _, v := range res.Indicators.Quote[0].Close {
			if v != nil {
				prices = append(prices, *v)
			}
		}
	}
	price = res.Meta.RegularMarketPrice
	if price == 0 && len(prices) > 0 {
		price = prices[len(prices)-1]
	}
	prev = res.Meta.PreviousClose
	if prev == 0 {
		prev = res.Meta.ChartPreviousClose
	}
	chartPrev = res.Meta.ChartPreviousClose
	regStart = res.Meta.CurrentTradingPeriod.Regular.Start
	regEnd = res.Meta.CurrentTradingPeriod.Regular.End
	return
}

func parseGtimgRows(rows []string) []float64 {
	prices := make([]float64, 0, len(rows))
	for _, row := range rows {
		parts := strings.Fields(row)
		if len(parts) < 2 {
			continue
		}
		p, err := strconv.ParseFloat(parts[1], 64)
		if err != nil {
			continue
		}
		prices = append(prices, p)
	}
	if len(prices) < 2 {
		return nil
	}
	return prices
}

func fetchGtimgMinute(code string) []float64 {
	isUS := strings.HasPrefix(code, "us")
	var urlStr string
	if isUS {
		urlStr = "https://web.ifzq.gtimg.cn/appstock/app/UsMinute/query?code=" + url.QueryEscape(code)
	} else {
		urlStr = "https://web.ifzq.gtimg.cn/appstock/app/minute/query?code=" + code
	}
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0")
	if isUS {
		req.Header.Set("Referer", "https://gu.qq.com/")
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return nil
	}
	defer resp.Body.Close()
	var j map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&j); err != nil {
		return nil
	}
	if isUS {
		if codeVal, ok := j["code"].(float64); ok && codeVal != 0 {
			return nil
		}
	}
	dataMap, _ := j["data"].(map[string]interface{})
	item, _ := dataMap[code].(map[string]interface{})
	d, _ := item["data"].(map[string]interface{})
	rowsIf, _ := d["data"].([]interface{})
	rows := make([]string, 0, len(rowsIf))
	for _, r := range rowsIf {
		if s, ok := r.(string); ok {
			rows = append(rows, s)
		}
	}
	prices := parseGtimgRows(rows)
	if isUS && prices != nil {
		filtered := prices[:0]
		for _, p := range prices {
			if p > 0 {
				filtered = append(filtered, p)
			}
		}
		if len(filtered) < 2 {
			return nil
		}
		prices = filtered
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
	prices     []float64
	timestamps []int64
	regStart   int64
	regEnd     int64
	chartPrev  float64
	yPrice     float64
	yPrev      float64
}

func refreshData() []StockData {
	configs := neededStocks()
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
				res.prices = fetchGtimgMinute(cfg.Code)
			case "yahoo":
				res.prices, res.timestamps, res.regStart, res.regEnd, res.yPrice, res.yPrev, res.chartPrev = fetchYahoo(cfg.Code)
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
		if !isChart {
			prices = nil
		} else {
			histMutex.Lock()
			h := priceHist[code]
			if price > 0 {
				h = append(h, price)
				if len(h) > 1440 {
					h = h[len(h)-1440:]
				}
				priceHist[code] = h
			}
			if len(prices) < 2 && len(h) > 2 {
				prices = append([]float64(nil), h...)
			}
			histMutex.Unlock()
		}

		items = append(items, StockData{
			Code: code, Name: c.Name, Group: c.Group,
			Price: price, Change: change, Pct: pct, Prev: prev,
			Prices: prices,
			Timestamps: cRes.timestamps, RegularStart: cRes.regStart, RegularEnd: cRes.regEnd, ChartPrevClose: cRes.chartPrev,
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
		return nil
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
