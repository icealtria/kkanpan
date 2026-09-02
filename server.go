package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"image"
	"image/png"
	"log"
	"net/http"
	"strings"
	"time"
)

func updateKindleScreen(img *image.Gray, fullRefresh bool) error {
	return screenDiffer.UpdateScreen(img, fullRefresh)
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
