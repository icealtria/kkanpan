package main

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"image/png"
	"log"
	"net/http"
	"strings"
	"time"
)

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

	http.HandleFunc("/style", func(w http.ResponseWriter, r *http.Request) {
		if m := r.URL.Query().Get("mode"); m != "" {
			SetStyleMode(m)
		} else {
			m := NextStyleMode()
			log.Printf("Style switched to: %s", m)
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
		reqView := r.URL.Query().Get("view")
		if reqView != "" {
			SetViewMode(reqView)
		}

		d := getData()
		img := renderScreenImage(d, 1072, 1448)
		var buf bytes.Buffer
		_ = png.Encode(&buf, img)
		b64 := base64.StdEncoding.EncodeToString(buf.Bytes())

		viewMode := GetViewMode()
		tabs := ComputeTabLayout(1072)
		styleBtnAbsX := 1072 + styleBtnX
		exitBtnAbsX := 1072 + exitBtnX

		var overlays strings.Builder
		for _, t := range tabs {
			selected := viewMode == t.Key
			if selected {
				overlays.WriteString(fmt.Sprintf(
					`<div style="position:absolute;left:%dpx;top:%dpx;width:%dpx;height:%dpx;background:#000"></div>`,
					t.X, t.Y, t.W, t.H))
			} else {
				overlays.WriteString(fmt.Sprintf(
					`<a href="/switch?view=%s" style="position:absolute;left:%dpx;top:%dpx;width:%dpx;height:%dpx"></a>`,
					t.Key, t.X, t.Y, t.W, t.H))
			}
		}
		overlays.WriteString(fmt.Sprintf(
			`<a href="/style" style="position:absolute;left:%dpx;top:%dpx;width:%dpx;height:%dpx"></a>`,
			styleBtnAbsX, styleBtnY, styleBtnW, styleBtnH))
		overlays.WriteString(fmt.Sprintf(
			`<a href="/exit" style="position:absolute;left:%dpx;top:%dpx;width:%dpx;height:%dpx"></a>`,
			exitBtnAbsX, exitBtnY, exitBtnW, exitBtnH))

		html := fmt.Sprintf(`<!DOCTYPE html><html><head><meta charset="utf-8"><meta http-equiv="refresh" content="60"><meta name="viewport" content="width=1072, initial-scale=1"><title>kkanpan</title><style>*{margin:0;padding:0;box-sizing:border-box}body{background:#eee;display:flex;justify-content:center;padding:8px}.wrap{position:relative;max-width:1072px;width:100%%}img{width:100%%;height:auto;background:#fff;box-shadow:0 2px 8px rgba(0,0,0,.2);display:block}a{cursor:pointer}</style></head><body><div class="wrap"><img src="data:image/png;base64,%s" alt="kkanpan">%s</div></body></html>`,
			b64, overlays.String())
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("Cache-Control", "no-cache")
		w.Write([]byte(html))
	})

	addr := fmt.Sprintf("%s:%d", host, port)
	log.Printf("HTTP Server running on http://%s", addr)
	if err := http.ListenAndServe(addr, nil); err != nil {
		log.Fatalf("HTTP server error: %v", err)
	}
}
