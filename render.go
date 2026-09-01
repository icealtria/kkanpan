package main

import (
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"math"
	"strings"
	"time"
)

// sparkline 工具 — DRY: 归一化与固定开盘-收盘宽度
func sparklinePoints(prices []float64, code string, w, h int) (pts []string, minVal, maxVal, rng float64) {
	if len(prices) < 2 {
		return nil, 0, 0, 1
	}
	minVal, maxVal = prices[0], prices[0]
	for _, p := range prices {
		if p < minVal {
			minVal = p
		}
		if p > maxVal {
			maxVal = p
		}
	}
	rng = maxVal - minVal
	if rng == 0 {
		rng = 1
	}
	total := tradingMinutes(code)
	denom := float64(len(prices) - 1)
	if total > 0 && len(prices) <= total {
		denom = float64(total - 1)
	}
	for i, p := range prices {
		x := 2.0 + float64(i)*float64(w-4)/denom
		y := 2.0 + (maxVal-p)*float64(h-4)/rng
		pts = append(pts, fmt.Sprintf("%.1f,%.1f", x, y))
	}
	return pts, minVal, maxVal, rng
}

func svgSparkline(prices []float64, w, h int, code string) string {
	if len(prices) < 2 {
		return ""
	}
	pts, _, _, _ := sparklinePoints(prices, code, w, h)
	poly := strings.Join(pts, " ")
	return fmt.Sprintf(`<svg width="%d" height="%d" viewBox="0 0 %d %d" xmlns="http://www.w3.org/2000/svg" shape-rendering="crispEdges">
  <rect width="%d" height="%d" fill="white" stroke="black" stroke-width="1"/>
  <polyline fill="none" stroke="black" stroke-width="1.5" points="%s"/>
</svg>`, w, h, w, h, w, h, poly)
}

// E-Ink 位图引擎 — Kindle eips 用
func drawLine(img *image.Gray, x0, y0, x1, y1 int, col uint8, width int) {
	dx := int(math.Abs(float64(x1 - x0)))
	dy := int(math.Abs(float64(y1 - y0)))
	sx, sy := 1, 1
	if x0 >= x1 {
		sx = -1
	}
	if y0 >= y1 {
		sy = -1
	}
	err := dx - dy
	setThickPixel := func(cx, cy int) {
		for ox := -width / 2; ox <= width/2; ox++ {
			for oy := -width / 2; oy <= width/2; oy++ {
				px, py := cx + ox, cy + oy
				if px >= 0 && px < img.Rect.Dx() && py >= 0 && py < img.Rect.Dy() {
					img.SetGray(px, py, color.Gray{Y: col})
				}
			}
		}
	}
	for {
		setThickPixel(x0, y0)
		if x0 == x1 && y0 == y1 {
			break
		}
		e2 := 2 * err
		if e2 > -dy {
			err -= dy
			x0 += sx
		}
		if e2 < dx {
			err += dx
			y0 += sy
		}
	}
}

func drawRect(img *image.Gray, x, y, w, h int, col uint8, border int) {
	for bx := 0; bx < border; bx++ {
		for i := x; i < x+w; i++ {
			if i >= 0 && i < img.Rect.Dx() {
				if y+bx < img.Rect.Dy() {
					img.SetGray(i, y+bx, color.Gray{Y: col})
				}
				if y+h-1-bx >= 0 && y+h-1-bx < img.Rect.Dy() {
					img.SetGray(i, y+h-1-bx, color.Gray{Y: col})
				}
			}
		}
		for j := y; j < y+h; j++ {
			if j >= 0 && j < img.Rect.Dy() {
				if x+bx < img.Rect.Dx() {
					img.SetGray(x+bx, j, color.Gray{Y: col})
				}
				if x+w-1-bx >= 0 && x+w-1-bx < img.Rect.Dx() {
					img.SetGray(x+w-1-bx, j, color.Gray{Y: col})
				}
			}
		}
	}
}

func fillRect(img *image.Gray, x, y, w, h int, col uint8) {
	for j := y; j < y+h; j++ {
		for i := x; i < x+w; i++ {
			if i >= 0 && i < img.Rect.Dx() && j >= 0 && j < img.Rect.Dy() {
				img.SetGray(i, j, color.Gray{Y: col})
			}
		}
	}
}

var basicGlyphs = map[rune][]uint8{
	'0': {0x3c, 0x66, 0x6e, 0x76, 0x66, 0x66, 0x3c, 0x00},
	'1': {0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x3c, 0x00},
	'2': {0x3c, 0x66, 0x06, 0x0c, 0x18, 0x30, 0x7e, 0x00},
	'3': {0x3c, 0x66, 0x06, 0x1c, 0x06, 0x66, 0x3c, 0x00},
	'4': {0x0c, 0x1c, 0x34, 0x64, 0x7e, 0x04, 0x04, 0x00},
	'5': {0x7e, 0x60, 0x7c, 0x06, 0x06, 0x66, 0x3c, 0x00},
	'6': {0x1c, 0x30, 0x60, 0x7c, 0x66, 0x66, 0x3c, 0x00},
	'7': {0x7e, 0x06, 0x0c, 0x18, 0x30, 0x30, 0x30, 0x00},
	'8': {0x3c, 0x66, 0x66, 0x3c, 0x66, 0x66, 0x3c, 0x00},
	'9': {0x3c, 0x66, 0x66, 0x3e, 0x06, 0x0c, 0x38, 0x00},
	'.': {0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00},
	'+': {0x00, 0x18, 0x18, 0x7e, 0x18, 0x18, 0x00, 0x00},
	'-': {0x00, 0x00, 0x00, 0x7e, 0x00, 0x00, 0x00, 0x00},
	'%': {0x62, 0x64, 0x08, 0x10, 0x20, 0x26, 0x46, 0x00},
	':': {0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00},
	'/': {0x02, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x40, 0x00},
	'[': {0x1e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1e, 0x00},
	']': {0x78, 0x18, 0x18, 0x18, 0x18, 0x18, 0x78, 0x00},
	'(': {0x0c, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0c, 0x00},
	')': {0x30, 0x18, 0x0c, 0x0c, 0x0c, 0x18, 0x30, 0x00},
	'A': {0x18, 0x3c, 0x66, 0x7e, 0x66, 0x66, 0x66, 0x00},
	'B': {0x7c, 0x66, 0x66, 0x7c, 0x66, 0x66, 0x7c, 0x00},
	'C': {0x3c, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3c, 0x00},
	'D': {0x78, 0x6c, 0x66, 0x66, 0x66, 0x6c, 0x78, 0x00},
	'E': {0x7e, 0x60, 0x60, 0x7c, 0x60, 0x60, 0x7e, 0x00},
	'F': {0x7e, 0x60, 0x60, 0x7c, 0x60, 0x60, 0x60, 0x00},
	'G': {0x3c, 0x66, 0x60, 0x6e, 0x66, 0x66, 0x3a, 0x00},
	'H': {0x66, 0x66, 0x66, 0x7e, 0x66, 0x66, 0x66, 0x00},
	'I': {0x3c, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3c, 0x00},
	'K': {0x66, 0x6c, 0x78, 0x70, 0x78, 0x6c, 0x66, 0x00},
	'L': {0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7e, 0x00},
	'M': {0x63, 0x77, 0x7f, 0x6b, 0x63, 0x63, 0x63, 0x00},
	'N': {0x66, 0x76, 0x7e, 0x7e, 0x6e, 0x66, 0x66, 0x00},
	'O': {0x3c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00},
	'P': {0x7c, 0x66, 0x66, 0x7c, 0x60, 0x60, 0x60, 0x00},
	'R': {0x7c, 0x66, 0x66, 0x7c, 0x78, 0x6c, 0x66, 0x00},
	'S': {0x3c, 0x66, 0x60, 0x3c, 0x06, 0x66, 0x3c, 0x00},
	'T': {0x7e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00},
	'U': {0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00},
	'V': {0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x18, 0x00},
	'W': {0x63, 0x63, 0x63, 0x6b, 0x7f, 0x77, 0x63, 0x00},
	'X': {0x66, 0x66, 0x3c, 0x18, 0x3c, 0x66, 0x66, 0x00},
	'Y': {0x66, 0x66, 0x66, 0x3c, 0x18, 0x18, 0x18, 0x00},
	'Z': {0x7e, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x7e, 0x00},
	'^': {0x18, 0x3c, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00},
	'v': {0x00, 0x00, 0x00, 0x66, 0x3c, 0x18, 0x00, 0x00},
	'=': {0x00, 0x7e, 0x00, 0x7e, 0x00, 0x00, 0x00, 0x00},
	'|': {0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00},
	' ': {0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00},
}

func drawChar(img *image.Gray, x, y int, ch rune, scale int, col uint8) int {
	glyph, ok := basicGlyphs[ch]
	if !ok {
		chUpper := rune(strings.ToUpper(string(ch))[0])
		glyph, ok = basicGlyphs[chUpper]
		if !ok {
			glyph = basicGlyphs[' ']
		}
	}
	for row, b := range glyph {
		for colIdx := 0; colIdx < 8; colIdx++ {
			if (b & (0x80 >> colIdx)) != 0 {
				for sx := 0; sx < scale; sx++ {
					for sy := 0; sy < scale; sy++ {
						px := x + colIdx*scale + sx
						py := y + row*scale + sy
						if px >= 0 && px < img.Rect.Dx() && py >= 0 && py < img.Rect.Dy() {
							img.SetGray(px, py, color.Gray{Y: col})
						}
					}
				}
			}
		}
	}
	return 8 * scale
}

func drawString(img *image.Gray, x, y int, text string, scale int, col uint8) int {
	curX := x
	for _, ch := range text {
		adv := drawChar(img, curX, y, ch, scale, col)
		curX += adv
	}
	return curX - x
}

func drawSparklineGraph(img *image.Gray, prices []float64, x, y, w, h int, code string) {
	if len(prices) < 2 {
		return
	}
	drawRect(img, x, y, w, h, 0, 1)
	_, minVal, maxVal, rng := sparklinePoints(prices, code, w, h)
	midY := y + 2 + int((maxVal-(minVal+maxVal)/2)*float64(h-4)/rng)
	for lx := x + 4; lx < x+w-4; lx += 6 {
		if lx+3 < x+w-4 && midY >= 0 && midY < img.Rect.Dy() {
			img.SetGray(lx, midY, color.Gray{Y: 128})
			img.SetGray(lx+1, midY, color.Gray{Y: 128})
		}
	}
	total := tradingMinutes(code)
	denom := float64(len(prices) - 1)
	if total > 0 && len(prices) <= total {
		denom = float64(total - 1)
	}
	for i := 0; i < len(prices)-1; i++ {
		x0 := x + 2 + int(float64(i)*float64(w-4)/denom)
		y0 := y + 2 + int((maxVal-prices[i])*float64(h-4)/rng)
		x1 := x + 2 + int(float64(i+1)*float64(w-4)/denom)
		y1 := y + 2 + int((maxVal-prices[i+1])*float64(h-4)/rng)
		drawLine(img, x0, y0, x1, y1, 0, 2)
	}
}

// 渲染 E-Ink 图像，支持右上角 [X] 关闭按钮与顶部 Tab
func renderScreenImage(data []StockData, width, height int) *image.Gray {
	img := image.NewGray(image.Rect(0, 0, width, height))
	draw.Draw(img, img.Bounds(), &image.Uniform{color.Gray{Y: 255}}, image.Point{}, draw.Src)

	viewMode := GetViewMode()
	effectiveGroup, isAuto := GetEffectiveGroup(viewMode)

	// 顶部 Header
	nowStr := time.Now().Format("2006-01-02 15:04:05")
	modeTag := fmt.Sprintf("[%s]", viewMode)
	if isAuto {
		modeTag = fmt.Sprintf("[AUTO: %s]", effectiveGroup)
	}
	drawString(img, 30, 15, "KKANPAN - KPW3", 3, 0)
	drawString(img, width-450, 18, modeTag, 2, 0)

	// 右上角 [X] 退出按钮 (高亮边框)
	drawRect(img, width-75, 10, 45, 34, 0, 2)
	drawString(img, width-60, 17, "X", 2, 0)

	// 绘制 5 个触控交互 Tab 栏 (Y: 52 ~ 88)
	tabs := []struct {
		Key   string
		Label string
	}{
		{"AUTO", "AUTO"},
		{"A股", "A-SHARE"},
		{"美股", "US-STOCK"},
		{"期货", "FUTURES"},
		{"全部", "ALL"},
	}

	tabCount := len(tabs)
	tabGap := 8
	tabTotalW := width - 60
	tabW := (tabTotalW - (tabCount-1)*tabGap) / tabCount
	tabY := 52
	tabH := 36

	for i, t := range tabs {
		tx := 30 + i*(tabW+tabGap)
		selected := (viewMode == t.Key)
		if selected {
			fillRect(img, tx, tabY, tabW, tabH, 0)
			padX := (tabW - len(t.Label)*16) / 2
			drawString(img, tx+padX, tabY+10, t.Label, 2, 255)
		} else {
			drawRect(img, tx, tabY, tabW, tabH, 0, 2)
			padX := (tabW - len(t.Label)*16) / 2
			drawString(img, tx+padX, tabY+10, t.Label, 2, 0)
		}
	}

	drawLine(img, 30, 96, width-30, 96, 0, 2)

	// 分组数据
	groups := make(map[string][]StockData)
	for _, d := range data {
		groups[d.Group] = append(groups[d.Group], d)
	}

	startY := 110

	// 单组大视图模式 vs 全部概览模式
	if effectiveGroup != "全部" && len(groups[effectiveGroup]) > 0 {
		list := groups[effectiveGroup]
		fillRect(img, 30, startY, width-60, 36, 0)
		gTitle := fmt.Sprintf("=== %s FOCUS VIEW (%d STOCKS) ===", effectiveGroup, len(list))
		drawString(img, 45, startY+10, gTitle, 2, 255)
		startY += 45

		for _, item := range list {
			cardH := 65
			drawRect(img, 30, startY, width-60, cardH, 0, 2)
			label := item.Code
			if item.Name != "" {
				label = fmt.Sprintf("%-8s %s", item.Code, item.Name)
			}
			drawString(img, 45, startY+22, label, 2, 0)

			arrow := " "
			if item.Change > 0 {
				arrow = "^"
			} else if item.Change < 0 {
				arrow = "v"
			}
			priceStr := "--"
			if item.Price > 0 {
				priceStr = fmt.Sprintf("%.2f", item.Price)
			}
			chgStr := fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, item.Change, item.Pct)

			if len(item.Prices) > 2 {
				drawSparklineGraph(img, item.Prices, 300, startY+10, 420, 45, item.Code)
			}

			priceW := len(priceStr) * 8 * 3
			chgW := len(chgStr) * 8 * 2
			drawString(img, width-45-priceW, startY+10, priceStr, 3, 0)
			drawString(img, width-45-chgW, startY+38, chgStr, 2, 0)

			startY += cardH + 6
			if startY > height-80 {
				break
			}
		}
	} else {
		for _, gName := range []string{"A股", "美股", "期货"} {
			list, ok := groups[gName]
			if !ok || len(list) == 0 {
				continue
			}
			fillRect(img, 30, startY, width-60, 32, 0)
			gLabel := gName
			if gName == "A股" {
				gLabel = "A-SHARE"
			} else if gName == "美股" {
				gLabel = "US-STOCK"
			} else if gName == "期货" {
				gLabel = "FUTURES"
			}
			drawString(img, 45, startY+8, "[ "+gLabel+" ]", 2, 255)
			startY += 38

			for _, item := range list {
				cardH := 65
				drawRect(img, 30, startY, width-60, cardH, 0, 2)
				label := item.Code
				if item.Name != "" {
					label = fmt.Sprintf("%-8s %s", item.Code, item.Name)
				}
				drawString(img, 45, startY+22, label, 2, 0)

				arrow := " "
				if item.Change > 0 {
					arrow = "^"
				} else if item.Change < 0 {
					arrow = "v"
				}
				priceStr := "--"
				if item.Price > 0 {
					priceStr = fmt.Sprintf("%.2f", item.Price)
				}
				chgStr := fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, item.Change, item.Pct)

				if len(item.Prices) > 2 {
					drawSparklineGraph(img, item.Prices, 300, startY+10, 420, 45, item.Code)
				}

				priceW := len(priceStr) * 8 * 3
				chgW := len(chgStr) * 8 * 2
				drawString(img, width-45-priceW, startY+10, priceStr, 3, 0)
				drawString(img, width-45-chgW, startY+38, chgStr, 2, 0)

				startY += cardH + 6
				if startY > height-80 {
					break
				}
			}
			startY += 6
		}
	}

	// ponytail: AUTO 模式期货常驻卡片（与 ALL 同尺寸 65），非 ticker
	if isAuto && effectiveGroup != "全部" && effectiveGroup != "期货" {
		if futs, ok := groups["期货"]; ok && len(futs) > 0 {
			fillRect(img, 30, startY, width-60, 32, 0)
			drawString(img, 45, startY+8, "[ FUTURES ]", 2, 255)
			startY += 38
			for _, f := range futs {
				if startY > height-80 {
					break
				}
				cardH := 65
				drawRect(img, 30, startY, width-60, cardH, 0, 2)
				label := f.Code
				if f.Name != "" {
					label = fmt.Sprintf("%-8s %s", f.Code, f.Name)
				}
				drawString(img, 45, startY+22, label, 2, 0)
				arrow := " "
				if f.Change > 0 {
					arrow = "^"
				} else if f.Change < 0 {
					arrow = "v"
				}
				priceStr := "--"
				if f.Price > 0 {
					priceStr = fmt.Sprintf("%.2f", f.Price)
				}
				chgStr := fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, f.Change, f.Pct)
				priceW := len(priceStr) * 8 * 3
				chgW := len(chgStr) * 8 * 2
				drawString(img, width-45-priceW, startY+10, priceStr, 3, 0)
				drawString(img, width-45-chgW, startY+38, chgStr, 2, 0)
				// 期货无分时，不画线
				startY += cardH + 6
			}
		}
	}

	// 底部触控提示栏
	drawString(img, 30, height-40, "TAP [X] TO EXIT | TAP TABS TO SWITCH | "+nowStr, 2, 100)
	return img
}

// 渲染 SVG 矢量图
func renderScreenSVG(data []StockData, width, height int) string {
	viewMode := GetViewMode()
	effectiveGroup, isAuto := GetEffectiveGroup(viewMode)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf(`<svg width="%d" height="%d" viewBox="0 0 %d %d" xmlns="http://www.w3.org/2000/svg" shape-rendering="crispEdges">`, width, height, width, height))
	sb.WriteString(`<rect width="100%" height="100%" fill="white"/>`)
	nowStr := time.Now().Format("2006-01-02 15:04:05")

	modeTag := fmt.Sprintf("[%s]", viewMode)
	if isAuto {
		modeTag = fmt.Sprintf("[AUTO: %s]", effectiveGroup)
	}
	sb.WriteString(fmt.Sprintf(`<text x="30" y="38" font-family="monospace" font-size="26" font-weight="bold">KKANPAN - KPW3</text>`))
	sb.WriteString(fmt.Sprintf(`<text x="%d" y="38" font-family="monospace" font-size="20" font-weight="bold" text-anchor="end">%s</text>`, width-90, modeTag))

	// 右上角 [X] 关闭按钮
	sb.WriteString(fmt.Sprintf(`<a href="/exit"><rect x="%d" y="10" width="45" height="34" fill="white" stroke="black" stroke-width="2"/><text x="%d" y="34" font-family="monospace" font-size="20" font-weight="bold" text-anchor="middle">X</text></a>`, width-75, width-52))

	// Tab 栏
	tabs := []struct {
		Key   string
		Label string
	}{
		{"AUTO", "AUTO"},
		{"A股", "A-SHARE"},
		{"美股", "US-STOCK"},
		{"期货", "FUTURES"},
		{"全部", "ALL"},
	}
	tabCount := len(tabs)
	tabGap := 8
	tabTotalW := width - 60
	tabW := (tabTotalW - (tabCount-1)*tabGap) / tabCount
	tabY := 52
	tabH := 36

	for i, t := range tabs {
		tx := 30 + i*(tabW+tabGap)
		selected := (viewMode == t.Key)
		if selected {
			sb.WriteString(fmt.Sprintf(`<a href="/switch?view=%s"><rect x="%d" y="%d" width="%d" height="%d" fill="black"/><text x="%d" y="%d" font-family="monospace" font-size="16" fill="white" text-anchor="middle">%s</text></a>`, t.Key, tx, tabY, tabW, tabH, tx+tabW/2, tabY+24, t.Label))
		} else {
			sb.WriteString(fmt.Sprintf(`<a href="/switch?view=%s"><rect x="%d" y="%d" width="%d" height="%d" fill="white" stroke="black" stroke-width="2"/><text x="%d" y="%d" font-family="monospace" font-size="16" fill="black" text-anchor="middle">%s</text></a>`, t.Key, tx, tabY, tabW, tabH, tx+tabW/2, tabY+24, t.Label))
		}
	}

	sb.WriteString(fmt.Sprintf(`<line x1="30" y1="96" x2="%d" y2="96" stroke="black" stroke-width="2"/>`, width-30))

	groups := make(map[string][]StockData)
	for _, d := range data {
		groups[d.Group] = append(groups[d.Group], d)
	}

	startY := 110
	if effectiveGroup != "全部" && len(groups[effectiveGroup]) > 0 {
		list := groups[effectiveGroup]
		sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="36" fill="black"/>`, startY, width-60))
		sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="18" fill="white">=== %s FOCUS VIEW (%d STOCKS) ===</text>`, startY+24, effectiveGroup, len(list)))
		startY += 45

		for _, item := range list {
			cardH := 65
			sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="%d" fill="none" stroke="black" stroke-width="2"/>`, startY, width-60, cardH))
			label := item.Code
			if item.Name != "" {
				label = fmt.Sprintf("%-8s %s", item.Code, item.Name)
			}
			sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="16">%s</text>`, startY+30, label))

			arrow := ""
			if item.Change > 0 {
				arrow = "^"
			} else if item.Change < 0 {
				arrow = "v"
			}
			priceStr := "--"
			if item.Price > 0 {
				priceStr = fmt.Sprintf("%.2f", item.Price)
			}
			chgStr := fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, item.Change, item.Pct)

			if len(item.Prices) > 2 {
				pts, _, _, _ := sparklinePoints(item.Prices, item.Code, 420, 45)
				gx, gy := 300, startY+10
				var shifted []string
				for _, pt := range pts {
					parts := strings.Split(pt, ",")
					if len(parts) == 2 {
						x := fmt.Sprintf("%.1f", mustParseFloat(parts[0])+float64(gx))
						y := fmt.Sprintf("%.1f", mustParseFloat(parts[1])+float64(gy))
						shifted = append(shifted, x+","+y)
					}
				}
				sb.WriteString(fmt.Sprintf(`<rect x="%d" y="%d" width="%d" height="45" fill="none" stroke="black" stroke-width="1"/>`, gx, gy))
				sb.WriteString(fmt.Sprintf(`<polyline fill="none" stroke="black" stroke-width="1.5" points="%s"/>`, strings.Join(shifted, " ")))
			}

			sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="22" font-weight="bold" text-anchor="end">%s</text>`, width-30, startY+28, priceStr))
			sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="14" text-anchor="end">%s</text>`, width-30, startY+48, chgStr))
			startY += cardH + 6
			if startY > height-80 {
				break
			}
		}
	} else {
		for _, gName := range []string{"A股", "美股", "期货"} {
			list, ok := groups[gName]
			if !ok || len(list) == 0 {
				continue
			}
			gLabel := gName
			if gName == "A股" {
				gLabel = "A-SHARE"
			} else if gName == "美股" {
				gLabel = "US-STOCK"
			} else if gName == "期货" {
				gLabel = "FUTURES"
			}
			sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="32" fill="black"/>`, startY, width-60))
			sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="16" fill="white">[ %s ]</text>`, startY+22, gLabel))
			startY += 38

			for _, item := range list {
				cardH := 65
				sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="%d" fill="none" stroke="black" stroke-width="2"/>`, startY, width-60, cardH))
				label := item.Code
				if item.Name != "" {
					label = fmt.Sprintf("%-8s %s", item.Code, item.Name)
				}
				sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="16">%s</text>`, startY+30, label))

				arrow := ""
				if item.Change > 0 {
					arrow = "^"
				} else if item.Change < 0 {
					arrow = "v"
				}
				priceStr := "--"
				if item.Price > 0 {
					priceStr = fmt.Sprintf("%.2f", item.Price)
				}
				chgStr := fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, item.Change, item.Pct)

				if len(item.Prices) > 2 {
					pts, _, _, _ := sparklinePoints(item.Prices, item.Code, 420, 45)
					gx, gy := 300, startY+10
					var shifted []string
					for _, pt := range pts {
						parts := strings.Split(pt, ",")
						if len(parts) == 2 {
							x := fmt.Sprintf("%.1f", mustParseFloat(parts[0])+float64(gx))
							y := fmt.Sprintf("%.1f", mustParseFloat(parts[1])+float64(gy))
							shifted = append(shifted, x+","+y)
						}
					}
					sb.WriteString(fmt.Sprintf(`<rect x="%d" y="%d" width="%d" height="45" fill="none" stroke="black" stroke-width="1"/>`, gx, gy))
					sb.WriteString(fmt.Sprintf(`<polyline fill="none" stroke="black" stroke-width="1.5" points="%s"/>`, strings.Join(shifted, " ")))
				}

				sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="22" font-weight="bold" text-anchor="end">%s</text>`, width-30, startY+28, priceStr))
				sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="14" text-anchor="end">%s</text>`, width-30, startY+48, chgStr))
				startY += cardH + 6
				if startY > height-80 {
					break
				}
			}
			startY += 6
		}
	}

	if isAuto && effectiveGroup != "全部" && effectiveGroup != "期货" {
		if futs, ok := groups["期货"]; ok && len(futs) > 0 {
			sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="32" fill="black"/>`, startY, width-60))
			sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="16" fill="white">[ FUTURES ]</text>`, startY+22))
			startY += 38
			for _, f := range futs {
				if startY > height-80 {
					break
				}
				priceStr := "--"
				if f.Price > 0 {
					priceStr = fmt.Sprintf("%.2f", f.Price)
				}
				arrow := ""
				if f.Change > 0 {
					arrow = "^"
				} else if f.Change < 0 {
					arrow = "v"
				}
				chgStr := fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, f.Change, f.Pct)
				label := f.Code
				if f.Name != "" {
					label = fmt.Sprintf("%-8s %s", f.Code, f.Name)
				}
				sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="65" fill="none" stroke="black" stroke-width="2"/>`, startY, width-60))
				sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="16">%s</text>`, startY+30, label))
				sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="22" font-weight="bold" text-anchor="end">%s</text>`, width-30, startY+28, priceStr))
				sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="14" text-anchor="end">%s</text>`, width-30, startY+48, chgStr))
				startY += 71
			}
		}
	}

	sb.WriteString(fmt.Sprintf(`<text x="30" y="%d" font-family="monospace" font-size="16" fill="#666">TAP [X] TO EXIT | TAP TABS TO SWITCH | %s</text>`, height-28, nowStr))
	sb.WriteString(`</svg>`)
	return sb.String()
}

func mustParseFloat(s string) float64 {
	var v float64
	fmt.Sscanf(s, "%f", &v)
	return v
}
