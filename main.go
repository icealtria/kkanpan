package main

import (
	"flag"
	"log"
	"math"
	"time"
)

func dataEqual(a, b []StockData) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i].Code != b[i].Code ||
			math.Abs(a[i].Price-b[i].Price) > 1e-9 ||
			math.Abs(a[i].Change-b[i].Change) > 1e-9 ||
			math.Abs(a[i].Pct-b[i].Pct) > 1e-9 {
			return false
		}
	}
	return true
}

func main() {
	port := flag.Int("port", 8000, "HTTP port (requires -http)")
	host := flag.String("host", "0.0.0.0", "HTTP listen addr")
	interval := flag.Int("interval", 60, "refresh interval (seconds)")
	width := flag.Int("width", 1072, "screen width (KPW3)")
	height := flag.Int("height", 1448, "screen height (KPW3)")
	eips := flag.Bool("eips", true, "enable eips direct e-ink refresh (default on)")
	once := flag.Bool("once", false, "single refresh and exit (for deep sleep script)")
	web := flag.Bool("http", false, "enable HTTP server (off by default)")
	initialView := flag.String("view", "", "initial view mode (AUTO, ALL or group from stocks.json, default from app.json)")
	flag.Parse()

	appConfig = loadAppConfig()
	initClients()
	view := *initialView
	if view == "" {
		view = GetDefaultView()
	}
	SetViewMode(view)
	log.Printf("Starting kkanpan for Kindle KPW3 (ViewMode: %s)...", view)

	// 共存模式: 不杀 framework, 通过 pillow+awesome+wmctrl+statusbar 屏蔽状态栏 (KOReader 同款, 退出不重启)
	DisableCoexistMode()
	defer EnableCoexistMode()

	data := refreshData()
	log.Printf("Fetched %d stocks successfully", len(data))

	if *once {
		img := renderScreenImage(data, *width, *height)
		_ = screenDiffer.UpdateScreen(img, true)
		log.Println("Once mode completed.")
		return
	}

	if *web {
		go startHTTPServer(*host, *port)
	}

	go startTouchListener(*width, *height)

	if *eips {
		refreshCount := 0
		ticker := time.NewTicker(time.Duration(*interval) * time.Second)
		defer ticker.Stop()

		img := renderScreenImage(data, *width, *height)
		_ = screenDiffer.UpdateScreen(img, true)
		lastData := data
		lastView := GetViewMode()

		for {
			select {
			case <-ticker.C:
				refreshCount++
				d := refreshData()

				// 数据无变化且未切换 Tab → 跳过 render+diff, CPU 零消耗
				curView := GetViewMode()
				if dataEqual(lastData, d) && curView == lastView {
					log.Println("[skip] Data unchanged, skipping render")
					continue
				}
				lastData = d
				lastView = curView

				img := renderScreenImage(d, *width, *height)
				full := (refreshCount % 5) == 0
				if err := screenDiffer.UpdateScreen(img, full); err != nil {
					log.Printf("Screen update error: %v", err)
				}
			case <-triggerRefreshCh:
				// 切 Tab 后布局完全不同，清除 diff 缓存做全屏刷新
				log.Println("Instant refresh triggered by user interaction...")
				screenDiffer.ClearDiffCache()
				d := getData()
				img := renderScreenImage(d, *width, *height)
				_ = screenDiffer.UpdateScreen(img, false)
				lastData = d
				lastView = GetViewMode()
			}
		}
	} else {
		select {}
	}
}
