package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"image"
	"image/png"
	"log"
	"net/http"
	"os"
	"os/exec"
	"strings"
	"time"
)

func updateKindleScreen(img *image.Gray, fullRefresh bool) error {
	tmpPath := "/tmp/kkanpan.png"
	f, err := os.Create(tmpPath)
	if err != nil {
		return err
	}
	if err := png.Encode(f, img); err != nil {
		f.Close()
		return err
	}
	f.Close()

	eipsPath := "/usr/sbin/eips"
	if _, err := os.Stat(eipsPath); err == nil {
		if fullRefresh {
			_ = exec.Command(eipsPath, "-c").Run()
			time.Sleep(200 * time.Millisecond)
			// -f 触发全刷波形，-g 加载图片
			cmd := exec.Command(eipsPath, "-f", "-g", tmpPath)
			if out, err := cmd.CombinedOutput(); err != nil {
				log.Printf("eips -f -g err: %v, output: %s, retrying -g", err, string(out))
				_ = exec.Command(eipsPath, "-g", tmpPath).Run()
			}
		} else {
			cmd := exec.Command(eipsPath, "-g", tmpPath)
			if out, err := cmd.CombinedOutput(); err != nil {
				log.Printf("eips -g err: %v, output: %s", err, string(out))
			}
		}
		log.Println("Kindle eips refreshed successfully")
	} else {
		log.Printf("Screen image rendered to %s (not on Kindle device)", tmpPath)
	}
	return nil
}

func startHTTPServer(host string, port int) {
	http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("ok"))
	})

	http.HandleFunc("/api", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		d := getData()
		json.NewEncoder(w).Encode(d)
	})

	http.HandleFunc("/switch", func(w http.ResponseWriter, r *http.Request) {
		view := r.URL.Query().Get("view")
		if view != "" {
			SetViewMode(view)
			log.Printf("Remote view switch requested: %s", view)
		}
		ref := r.Header.Get("Referer")
		if ref != "" {
			http.Redirect(w, r, ref, 302)
		} else {
			http.Redirect(w, r, "/", 302)
		}
	})

	http.HandleFunc("/exit", func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("Exiting kkanpan... Restoring Kindle system."))
		go func() {
			time.Sleep(500 * time.Millisecond)
			quitApp()
		}()
	})

	http.HandleFunc("/screen.svg", func(w http.ResponseWriter, r *http.Request) {
		d := getData()
		svg := renderScreenSVG(d, 1072, 1448)
		w.Header().Set("Content-Type", "image/svg+xml; charset=utf-8")
		w.Header().Set("Cache-Control", "no-cache")
		w.Write([]byte(svg))
	})

	http.HandleFunc("/screen.png", func(w http.ResponseWriter, r *http.Request) {
		d := getData()
		img := renderScreenImage(d, 1072, 1448)
		var buf bytes.Buffer
		_ = png.Encode(&buf, img)
		w.Header().Set("Content-Type", "image/png")
		w.Header().Set("Cache-Control", "no-cache")
		w.Write(buf.Bytes())
	})

	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if strings.HasSuffix(r.URL.Path, ".svg") {
			http.Redirect(w, r, "/screen.svg", 302)
			return
		}
		reqView := r.URL.Query().Get("view")
		if reqView != "" {
			SetViewMode(reqView)
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("Cache-Control", "no-cache")
		d := getData()
		svg := renderScreenSVG(d, 1072, 1448)
		html := fmt.Sprintf(`<!DOCTYPE html><html><head><meta charset="utf-8"><meta http-equiv="refresh" content="60"><meta name="viewport" content="width=1072, initial-scale=1"><title>kkanpan</title><style>*{margin:0;padding:0;box-sizing:border-box}body{background:#eee;display:flex;justify-content:center;padding:8px}svg{max-width:1072px;width:100%%;height:auto;background:#fff;box-shadow:0 2px 8px rgba(0,0,0,.2)}</style></head><body>%s</body></html>`, svg)
		w.Write([]byte(html))
	})

	addr := fmt.Sprintf("%s:%d", host, port)
	log.Printf("HTTP Server running on http://%s", addr)
	if err := http.ListenAndServe(addr, nil); err != nil {
		log.Fatalf("HTTP server error: %v", err)
	}
}
