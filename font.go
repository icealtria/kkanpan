package main

import (
	"image"
	"image/color"
	"image/draw"
	"log"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"

	"golang.org/x/image/font"
	"golang.org/x/image/font/opentype"
	"golang.org/x/image/font/sfnt"
	"golang.org/x/image/math/fixed"
)

var (
	parsedFont   *sfnt.Font
	fontFaceMap  = make(map[int]font.Face)
	measureCache = make(map[string]int)
	fontMu       sync.RWMutex
	fontInitOnce sync.Once
)

func InitFont() {
	fontInitOnce.Do(func() {
		candidates := []string{
			// 1. 插件专属目录自定义字体
			"/mnt/us/extensions/kkanpan/font.ttf",
			"/mnt/us/extensions/kkanpan/font.otf",
			"font.ttf",
			"font.otf",

			// 2. Kindle 用户字体目录
			"/mnt/us/fonts/Kindle_Hei.ttf",
			"/mnt/us/fonts/Songti.ttf",
			"/mnt/us/fonts/font.ttf",

			// 3. Kindle 内部固件自带字体
			"/usr/java/lib/fonts/STHeitiMedium.ttf",
			"/usr/java/lib/fonts/STHeitiBold.ttf",
			"/usr/java/lib/fonts/STSongMedium.ttf",
			"/usr/java/lib/fonts/STSongBold.ttf",
			"/usr/java/lib/fonts/STHeitiTC.ttf",
			"/usr/java/lib/fonts/STSongTC.ttf",
			"/usr/java/lib/fonts/MTChineseSurrogates.ttf",
			"/usr/java/lib/fonts/code2000.ttf",

			// 4. 开发调试环境 (macOS / Linux)
			"/System/Library/Fonts/PingFang.ttc",
			"/System/Library/Fonts/STHeiti Light.ttc",
			"/System/Library/Fonts/STHeiti Medium.ttc",
			"/System/Library/Fonts/Supplemental/Songti.ttc",
			"/Library/Fonts/Arial Unicode.ttf",
			"/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
			"/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
		}

		if entries, err := os.ReadDir("/mnt/us/fonts"); err == nil {
			for _, e := range entries {
				ext := strings.ToLower(filepath.Ext(e.Name()))
				if ext == ".ttf" || ext == ".otf" || ext == ".ttc" {
					candidates = append([]string{filepath.Join("/mnt/us/fonts", e.Name())}, candidates...)
				}
			}
		}

		for _, p := range candidates {
			if data, err := os.ReadFile(p); err == nil && len(data) > 0 {
				var f *sfnt.Font
				if strings.HasSuffix(strings.ToLower(p), ".ttc") {
					if col, err := opentype.ParseCollection(data); err == nil && col.NumFonts() > 0 {
						f, _ = col.Font(0)
					}
				} else {
					f, _ = opentype.Parse(data)
				}
				if f != nil {
					parsedFont = f
					log.Printf("Successfully loaded Chinese font: %s", p)
					return
				}
			}
		}
		log.Println("No external TrueType Chinese font found, using built-in bitmap fallback.")
	})
}

func getFontFace(size int) font.Face {
	fontMu.RLock()
	if f, ok := fontFaceMap[size]; ok {
		fontMu.RUnlock()
		return f
	}
	fontMu.RUnlock()

	if parsedFont == nil {
		return nil
	}

	fontMu.Lock()
	defer fontMu.Unlock()
	if f, ok := fontFaceMap[size]; ok {
		return f
	}

	face, err := opentype.NewFace(parsedFont, &opentype.FaceOptions{
		Size:    float64(size),
		DPI:     72,
		Hinting: font.HintingFull,
	})
	if err != nil {
		log.Printf("Failed to create font face of size %d: %v", size, err)
		return nil
	}
	fontFaceMap[size] = face
	return face
}

func DrawText(dst draw.Image, x, y int, text string, size int, col color.Color) int {
	InitFont()
	face := getFontFace(size)
	if face != nil {
		metrics := face.Metrics()
		ascent := metrics.Ascent.Ceil()
		d := &font.Drawer{
			Dst:  dst,
			Src:  image.NewUniform(col),
			Face: face,
			Dot: fixed.Point26_6{
				X: fixed.I(x),
				Y: fixed.I(y + ascent),
			},
		}
		d.DrawString(text)
		return d.Dot.X.Ceil() - x
	}

	if grayImg, ok := dst.(*image.Gray); ok {
		scale := max(size/16, 1)
		grayCol := uint8(0)
		if r, g, b, _ := col.RGBA(); r > 0x8000 || g > 0x8000 || b > 0x8000 {
			grayCol = 255
		}
		return drawString(grayImg, x, y, text, scale, grayCol)
	}
	return 0
}

func MeasureText(text string, size int) int {
	key := strconv.Itoa(size) + "|" + text
	fontMu.RLock()
	if v, ok := measureCache[key]; ok {
		fontMu.RUnlock()
		return v
	}
	fontMu.RUnlock()
	InitFont()
	face := getFontFace(size)
	var w int
	if face != nil {
		d := &font.Drawer{Face: face}
		w = d.MeasureString(text).Ceil()
	} else {
		scale := max(size/16, 1)
		w = len(text) * 8 * scale
	}
	fontMu.Lock()
	measureCache[key] = w
	fontMu.Unlock()
	return w
}
