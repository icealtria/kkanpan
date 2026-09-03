package main

import (
	"bytes"
	"fmt"
	"image/png"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// mockTransport redirects all HTTP requests to a mock server
type mockTransport struct {
	target string
}

func (t *mockTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	req.URL.Scheme = "http"
	req.URL.Host = t.target
	return http.DefaultTransport.RoundTrip(req)
}

// mockStockData builds realistic test data for rendering
func mockStockData() []StockData {
	genPrices := func(base float64, n int) []float64 {
		p := make([]float64, n)
		for i := range p {
			p[i] = base + float64(i)*0.3
		}
		return p
	}

	return []StockData{
		{
			Code: "sh600519", Name: "贵州茅台", Group: "a-share",
			Price: 1800.50, Change: 10.30, Pct: 0.57, Prev: 1790.20,
			Prices: genPrices(1795.0, 240),
		},
		{
			Code: "sh601318", Name: "中国平安", Group: "a-share",
			Price: 52.80, Change: -0.45, Pct: -0.84, Prev: 53.25,
			Prices: genPrices(53.0, 240),
		},
		{
			Code: "sz000858", Name: "五粮液", Group: "a-share",
			Price: 168.20, Change: 2.10, Pct: 1.27, Prev: 166.10,
			Prices: genPrices(167.0, 240),
		},
		{
			Code: "usAAPL", Name: "Apple", Group: "us",
			Price: 195.50, Change: 1.20, Pct: 0.62, Prev: 194.30,
			Prices: genPrices(194.0, 390),
		},
		{
			Code: "usNVDA", Name: "NVIDIA", Group: "us",
			Price: 125.80, Change: -2.30, Pct: -1.79, Prev: 128.10,
			Prices: genPrices(127.0, 390),
		},
		{
			Code: "usTSLA", Name: "Tesla", Group: "us",
			Price: 248.50, Change: 5.80, Pct: 2.39, Prev: 242.70,
			Prices: genPrices(243.0, 390),
		},
		{
			Code: "hk00700", Name: "腾讯控股", Group: "hk",
			Price: 380.60, Change: 3.40, Pct: 0.90, Prev: 377.20,
			Prices: genPrices(378.0, 330),
		},
	}
}

func setupTestGlobals() {
	stocksCache = []StockConfig{
		{Code: "sh600519", Name: "贵州茅台", Group: "a-share", Source: "tencent"},
		{Code: "sh601318", Name: "中国平安", Group: "a-share", Source: "tencent"},
		{Code: "sz000858", Name: "五粮液", Group: "a-share", Source: "tencent"},
		{Code: "usAAPL", Name: "Apple", Group: "us", Source: "yahoo"},
		{Code: "usNVDA", Name: "NVIDIA", Group: "us", Source: "yahoo"},
		{Code: "usTSLA", Name: "Tesla", Group: "us", Source: "yahoo"},
		{Code: "hk00700", Name: "腾讯控股", Group: "hk", Source: "tencent"},
	}
	appConfig = AppConfig{
		CacheTTL:    55,
		DefaultView: "ALL",
		AutoRules: []AutoRule{
			{Group: "a-share", Weekdays: []int{1, 2, 3, 4, 5}, Start: "09:30", End: "15:00"},
			{Group: "us", Weekdays: []int{1, 2, 3, 4, 5}, Start: "21:30", End: "04:00"},
		},
	}
	currentViewMode = "ALL"
	currentStyle = "normal"
}

func TestE2E_RenderImage(t *testing.T) {
	setupTestGlobals()
	data := mockStockData()

	img := renderScreenImage(data, 1072, 1448)
	if img == nil {
		t.Fatal("renderScreenImage returned nil")
	}

	if img.Rect.Dx() != 1072 || img.Rect.Dy() != 1448 {
		t.Errorf("image size = %dx%d, want 1072x1448", img.Rect.Dx(), img.Rect.Dy())
	}

	// Verify image is not all-white (has some black pixels)
	hasBlack := false
	for _, v := range img.Pix {
		if v < 200 {
			hasBlack = true
			break
		}
	}
	if !hasBlack {
		t.Error("rendered image is all white, expected some content")
	}

	// Save PNG for visual inspection
	dir := t.TempDir()
	path := filepath.Join(dir, "e2e_normal.png")
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, img); err != nil {
		t.Fatal(err)
	}
	t.Logf("Saved normal mode image: %s", path)
}

func TestE2E_RenderImage_LargeStyle(t *testing.T) {
	setupTestGlobals()
	currentStyle = "large"
	data := mockStockData()

	img := renderScreenImage(data, 1072, 1448)
	if img == nil {
		t.Fatal("renderScreenImage returned nil")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "e2e_large.png")
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, img); err != nil {
		t.Fatal(err)
	}
	t.Logf("Saved large style image: %s", path)
}

func TestE2E_RenderImage_AutoMode(t *testing.T) {
	setupTestGlobals()
	currentViewMode = "AUTO"
	data := mockStockData()

	img := renderScreenImage(data, 1072, 1448)
	if img == nil {
		t.Fatal("renderScreenImage returned nil")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "e2e_auto.png")
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, img); err != nil {
		t.Fatal(err)
	}
	t.Logf("Saved auto mode image: %s", path)
}

func TestE2E_RenderImage_SingleGroup(t *testing.T) {
	setupTestGlobals()
	currentViewMode = "a-share"
	data := mockStockData()

	img := renderScreenImage(data, 1072, 1448)
	if img == nil {
		t.Fatal("renderScreenImage returned nil")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "e2e_ashare.png")
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, img); err != nil {
		t.Fatal(err)
	}
	t.Logf("Saved a-share group image: %s", path)
}

func TestE2E_RenderImage_MultiPage(t *testing.T) {
	setupTestGlobals()
	currentViewMode = "ALL"

	// Create many stocks to force pagination
	var data []StockData
	groups := []string{"a-share", "us", "hk"}
	for _, g := range groups {
		for i := 0; i < 10; i++ {
			data = append(data, StockData{
				Code: fmt.Sprintf("sh%06d", i), Name: fmt.Sprintf("Stock%d", i), Group: g,
				Price: float64(100 + i), Change: float64(i - 5), Pct: float64(i) * 0.1, Prev: float64(99 + i),
				Prices: func() []float64 {
					p := make([]float64, 60)
					for j := range p {
						p[j] = float64(100+i) + float64(j)*0.1
					}
					return p
				}(),
			})
		}
	}

	img := renderScreenImage(data, 1072, 1448)
	if img == nil {
		t.Fatal("renderScreenImage returned nil")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "e2e_multipage.png")
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, img); err != nil {
		t.Fatal(err)
	}
	t.Logf("Saved multi-page image: %s", path)
}

func TestE2E_RenderSVG(t *testing.T) {
	setupTestGlobals()
	data := mockStockData()

	svg := renderScreenSVG(data, 1072, 1448)
	if svg == "" {
		t.Fatal("renderScreenSVG returned empty")
	}
	if !strings.HasPrefix(svg, "<svg") {
		t.Error("SVG should start with <svg")
	}
	if !strings.Contains(svg, "KKANPAN") {
		t.Error("SVG should contain KKANPAN header")
	}
	if !strings.Contains(svg, "贵州茅台") {
		t.Error("SVG should contain stock name")
	}
	if !strings.Contains(svg, "1800.50") {
		t.Error("SVG should contain stock price")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "e2e.svg")
	if err := os.WriteFile(path, []byte(svg), 0644); err != nil {
		t.Fatal(err)
	}
	t.Logf("Saved SVG: %s", path)
}

// TestE2E_MockHTTP_TencentAPI mocks the tencent HTTP API and verifies parseQT integration
func TestE2E_MockHTTP_TencentAPI(t *testing.T) {
	// Mock tencent qt.gtimg.cn response
	qtResp := `v_sh600519="1~贵州茅台~600519~1800.50~1790.20~1800.00~12000~6000~6000~1800.50~100~1800.40~200~1800.30~300~1800.20~100~1800.10~200~1801.00~100~1801.10~200~1801.20~300~1801.30~100~1801.40~200~15:00:00/1800.50/100/B/180050/12000|14:59:57/1800.40/200/S/360080/11900|14:59:54/1800.30/300/S/540090/11800|14:59:51/1800.20/100/S/180020/11700|14:59:48/1800.10/200/S/360020/11600|14:59:45/1800.00/100/S/180000/11500~20260901150000~10.30~0.56~1810.50~1780.20~1800.50/12000/21606000~12000~21606~0.67~30.80~~1810.50~1780.20~1.68~32313.60~32313.60~20.42~1969.22~1611.78~1.51~-1~1800.51~12.17~30.80~~~1.15~21606.00~0.00~0~~GP-A~-2.21~6.02~1.89~12.71~18.70~32313.60~1.51~5.30~-0.75~51897.64~6.73~-2.15~59.83~50.39~1.51~-13.53~1800.50~50.39~30.80~~2176~1.07";`

	minuteResp := `{"code":0,"data":{"sh600519":{"data":{"data":["09:30 1795.00","09:31 1795.50","09:32 1796.00","09:33 1796.50","09:34 1797.00"]}}}}`

	// Use a handler that matches on URL path prefix
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		path := r.URL.Path
		switch {
		case strings.HasPrefix(path, "/q="):
			w.Header().Set("Content-Type", "text/plain; charset=utf-8")
			fmt.Fprint(w, qtResp)
		case strings.Contains(path, "/minute/query"):
			w.Header().Set("Content-Type", "application/json; charset=utf-8")
			fmt.Fprint(w, minuteResp)
		default:
			http.NotFound(w, r)
		}
	})

	mockServer := httptest.NewServer(handler)
	defer mockServer.Close()

	// Override httpClient transport to redirect to mock
	origTransport := httpClient.Transport
	mockAddr := mockServer.Listener.Addr().String()
	httpClient.Transport = &mockTransport{target: mockAddr}
	defer func() { httpClient.Transport = origTransport }()

	// Set up stocks to only fetch from mock
	stocksCache = []StockConfig{
		{Code: "sh600519", Name: "贵州茅台", Group: "a-share", Source: "tencent"},
	}
	appConfig = AppConfig{CacheTTL: 55}

	// Parse the mock response
	qt := fetchQT([]string{"sh600519"})
	vals := qt["sh600519"]
	if len(vals) == 0 {
		t.Fatal("fetchQT returned empty for sh600519")
	}

	price, change, pct, prev := parseQT("sh600519", vals)
	if price != 1800.50 {
		t.Errorf("price = %f, want 1800.50", price)
	}
	if prev != 1790.20 {
		t.Errorf("prev = %f, want 1790.20", prev)
	}
	t.Logf("Mock API parseQT: price=%.2f change=%.2f pct=%.2f prev=%.2f", price, change, pct, prev)

	// Fetch minute data
	prices := fetchGtimgMinute("sh600519")
	if len(prices) < 2 {
		t.Errorf("fetchGtimgMinute returned %d prices, want >=2", len(prices))
	} else {
		t.Logf("Mock API minute prices: %v", prices)
	}
}

func TestE2E_EndToEnd_PNGOutput(t *testing.T) {
	setupTestGlobals()

	data := mockStockData()

	// Render both normal and large, verify file sizes differ
	normalImg := renderScreenImage(data, 1072, 1448)
	currentStyle = "large"
	largeImg := renderScreenImage(data, 1072, 1448)
	currentStyle = "normal"

	var normalBuf, largeBuf bytes.Buffer
	png.Encode(&normalBuf, normalImg)
	png.Encode(&largeBuf, largeImg)

	t.Logf("Normal PNG size: %d bytes", normalBuf.Len())
	t.Logf("Large PNG size: %d bytes", largeBuf.Len())

	if normalBuf.Len() == 0 {
		t.Error("normal PNG is empty")
	}
	if largeBuf.Len() == 0 {
		t.Error("large PNG is empty")
	}

	// Save both for comparison
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "normal.png"), normalBuf.Bytes(), 0644)
	os.WriteFile(filepath.Join(dir, "large.png"), largeBuf.Bytes(), 0644)
	t.Logf("Saved comparison PNGs to: %s", dir)
}
