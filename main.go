package main

import (
	"flag"
	"log"
	"time"
)

func main() {
	port := flag.Int("port", 8000, "HTTP 端口 (需配合 -http)")
	host := flag.String("host", "0.0.0.0", "HTTP 监听地址")
	interval := flag.Int("interval", 60, "屏幕刷新间隔(秒)")
	width := flag.Int("width", 1072, "KPW3屏幕宽度")
	height := flag.Int("height", 1448, "KPW3屏幕高度")
	eips := flag.Bool("eips", true, "是否启用 eips 墨水屏直接刷屏 (默认开启)")
	once := flag.Bool("once", false, "执行一次刷新并退出 (适合深度休眠脚本)")
	web := flag.Bool("http", false, "是否开启 HTTP 网页服务 (默认关闭，Kindle 本机原生优先)")
	initialView := flag.String("view", "AUTO", "初始视图模式 (AUTO, A股, 美股, 期货, 全部)")
	flag.Parse()

	appConfig = loadAppConfig()
	initClients()
	SetViewMode(*initialView)
	log.Printf("Starting kkanpan for Kindle KPW3 (ViewMode: %s)...", *initialView)

	// 共存模式: 不杀 framework, 通过 pillow+awesome+wmctrl+statusbar 屏蔽状态栏 (KOReader 同款, 退出不重启)
	DisableCoexistMode()
	defer EnableCoexistMode()

	data := refreshData()
	log.Printf("Fetched %d stocks successfully", len(data))

	if *once {
		img := renderScreenImage(data, *width, *height)
		_ = updateKindleScreen(img, true)
		log.Println("Once mode completed.")
		// once 模式也需要恢复 UI (defer 会执行)
		return
	}

	// 默认关闭 HTTP，仅在显式指定 -http 时开启
	if *web {
		go startHTTPServer(*host, *port)
	}

	// 启动 Kindle 触控事件监听 (Tab 切换与右上角 [X] 关闭)
	go startTouchListener(*width, *height)

	if *eips {
		refreshCount := 0
		ticker := time.NewTicker(time.Duration(*interval) * time.Second)
		defer ticker.Stop()

		// 首次刷屏
		img := renderScreenImage(data, *width, *height)
		_ = updateKindleScreen(img, true)

		for {
			select {
			case <-ticker.C:
				refreshCount++
				d := refreshData()
				img := renderScreenImage(d, *width, *height)
				full := (refreshCount % 5) == 0
				if err := updateKindleScreen(img, full); err != nil {
					log.Printf("Screen update error: %v", err)
				}
			case <-triggerRefreshCh:
				// 用户触控切换 Tab 或点击刷新，立即响应重绘
				// 切 Tab 后布局完全不同，清除 diff 缓存做全屏刷新
				log.Println("Instant refresh triggered by user interaction...")
				screenDiffer.ClearDiffCache()
				d := getData()
				img := renderScreenImage(d, *width, *height)
				_ = updateKindleScreen(img, false)
			}
		}
	} else {
		select {}
	}
}
