package main

import (
	"flag"
	"log"
	"math"
	"time"
)

func dataChanged(a, b []StockData) bool {
	if len(a) != len(b) {
		return true
	}
	for i := range a {
		if a[i].Code != b[i].Code ||
			math.Abs(a[i].Price-b[i].Price) > 1e-9 ||
			math.Abs(a[i].Change-b[i].Change) > 1e-9 ||
			math.Abs(a[i].Pct-b[i].Pct) > 1e-9 {
			return true
		}
	}
	return false
}

func main() {
	port := flag.Int("port", 8000, "HTTP port (requires -http)")
	host := flag.String("host", "0.0.0.0", "HTTP listen addr")
	interval := flag.Int("interval", 60, "refresh interval (seconds)")
	width := flag.Int("width", 1072, "screen width (KPW3)")
	height := flag.Int("height", 1448, "screen height (KPW3)")
	once := flag.Bool("once", false, "single refresh and exit (for deep sleep script)")
	web := flag.Bool("http", false, "enable HTTP server (off by default)")
	initialView := flag.String("view", "", "initial view mode (AUTO, ALL or group from stocks.json, default from app.json)")
	flag.Parse()

	appConfig = loadAppConfig()
	initFileLog()
	initClients()
	view := *initialView
	if view == "" {
		view = GetDefaultView()
	}
	SetViewMode(view)
	log.Printf("Starting kkanpan for Kindle KPW3 (ViewMode: %s)...", view)

	DisableCoexistMode()
	if appConfig.DimFrontlight {
		SaveAndTurnOffFrontlight()
	}
	defer EnableCoexistMode()
	defer RestoreFrontlight()

	if err := initDisplay(); err != nil {
		log.Fatalf("Display init failed: %v", err)
	}
	defer closeDisplay()

	data := refreshData()
	UpdateDataRefreshTime()
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
	go startPowerButtonListener()

	refreshCount := 0
	ticker := time.NewTicker(time.Duration(*interval) * time.Second)
	defer ticker.Stop()

	img := renderScreenImage(data, *width, *height)
	_ = screenDiffer.UpdateScreen(img, true)
	lastData := data

	for {
		select {
		case <-ticker.C:
			refreshCount++
			d := refreshData()

			if dataChanged(lastData, d) {
				UpdateDataRefreshTime()
				img := renderScreenImage(d, *width, *height)
				full := (refreshCount % 5) == 0
				if err := screenDiffer.UpdateScreen(img, full); err != nil {
					log.Printf("Screen update error: %v", err)
				}
			}
			lastData = d
		case <-triggerRefreshCh:
			log.Println("Instant refresh triggered by user interaction...")
			screenDiffer.ClearDiffCache()
			d := getData()
			img := renderScreenImage(d, *width, *height)
			_ = screenDiffer.UpdateScreen(img, true)
		}
	}
}
