package main

import (
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"math"
	"strconv"
	"strings"
)

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
	denom := float64(len(prices) - 1)
	pts = make([]string, 0, len(prices))
	buf := make([]byte, 0, 32)
	for i, p := range prices {
		x := 2.0 + float64(i)*float64(w-4)/denom
		y := 2.0 + (maxVal-p)*float64(h-4)/rng
		buf = buf[:0]
		buf = strconv.AppendFloat(buf, x, 'f', 1, 64)
		buf = append(buf, ',')
		buf = strconv.AppendFloat(buf, y, 'f', 1, 64)
		pts = append(pts, string(buf))
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

func stockStrings(price, change, pct float64, isSVG bool) (priceStr, chgStr string) {
	if price > 0 {
		priceStr = fmt.Sprintf("%.2f", price)
	} else {
		priceStr = "--"
	}
	arrow := " "
	if isSVG {
		arrow = ""
	}
	if change > 0 {
		if isSVG {
			arrow = "^"
		} else {
			arrow = "▲"
		}
	} else if change < 0 {
		if isSVG {
			arrow = "v"
		} else {
			arrow = "▼"
		}
	}
	chgStr = fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, change, pct)
	return
}

func svgWriteCard(sb *strings.Builder, item StockData, startY, width, cardH int, withSparkline bool) {
	sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="%d" fill="none" stroke="black" stroke-width="2"/>`, startY, width-60, cardH))
	nameStr := item.Name
	if nameStr == "" {
		nameStr = item.Code
	}
	sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="26">%s</text>`, startY+28, nameStr))
	sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="18" fill="#666">%s</text>`, startY+62, item.Code))
	priceStr, chgStr := stockStrings(item.Price, item.Change, item.Pct, true)
	if withSparkline && len(item.Prices) > 2 {
		gx, gy := 240, startY+12
		sb.WriteString(fmt.Sprintf(`<rect x="%d" y="%d" width="480" height="70" fill="none" stroke="black" stroke-width="1"/>`, gx, gy))
		// 直接带偏移计算，避免 sparklinePoints→string→parse 往返
		prices := item.Prices
		minVal, maxVal := prices[0], prices[0]
		for _, p := range prices {
			if p < minVal {
				minVal = p
			}
			if p > maxVal {
				maxVal = p
			}
		}
		rng := maxVal - minVal
		if rng == 0 {
			rng = 1
		}
		denom := float64(len(prices) - 1)
		pts := make([]string, 0, len(prices))
		for i, p := range prices {
			x := float64(gx) + 2.0 + float64(i)*float64(480-4)/denom
			y := float64(gy) + 2.0 + (maxVal-p)*float64(70-4)/rng
			pts = append(pts, strconv.FormatFloat(x, 'f', 1, 64)+","+strconv.FormatFloat(y, 'f', 1, 64))
		}
		sb.WriteString(fmt.Sprintf(`<polyline fill="none" stroke="black" stroke-width="1.5" points="%s"/>`, strings.Join(pts, " ")))
	}
	chgY := startY + 64
	if withSparkline {
		chgY = startY + 66
	}
	sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="34" font-weight="bold" text-anchor="end">%s</text>`, width-45, startY+28, priceStr))
	sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="20" text-anchor="end">%s</text>`, width-45, chgY, chgStr))
}

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
	half := width / 2
	w := img.Rect.Dx()
	h := img.Rect.Dy()
	stride := img.Stride
	pix := img.Pix
	for {
		// 内联 setThickPixel, 直接写 Pix 避免 SetGray 开销
		for oy := -half; oy <= half; oy++ {
			py := y0 + oy
			if py < 0 || py >= h {
				continue
			}
			rowOff := py * stride
			for ox := -half; ox <= half; ox++ {
				px := x0 + ox
				if px < 0 || px >= w {
					continue
				}
				pix[rowOff+px] = col
			}
		}
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
	W := img.Rect.Dx()
	H := img.Rect.Dy()
	stride := img.Stride
	pix := img.Pix
	for bx := 0; bx < border; bx++ {
		yt := y + bx
		yb := y + h - 1 - bx
		if yt >= 0 && yt < H {
			off := yt * stride
			xs := x
			if xs < 0 {
				xs = 0
			}
			xe := x + w
			if xe > W {
				xe = W
			}
			for i := xs; i < xe; i++ {
				pix[off+i] = col
			}
		}
		if yb >= 0 && yb < H && yb != yt {
			off := yb * stride
			xs := x
			if xs < 0 {
				xs = 0
			}
			xe := x + w
			if xe > W {
				xe = W
			}
			for i := xs; i < xe; i++ {
				pix[off+i] = col
			}
		}
		xt := x + bx
		xr := x + w - 1 - bx
		if xt >= 0 && xt < W {
			ys := y
			if ys < 0 {
				ys = 0
			}
			ye := y + h
			if ye > H {
				ye = H
			}
			for j := ys; j < ye; j++ {
				pix[j*stride+xt] = col
			}
		}
		if xr >= 0 && xr < W && xr != xt {
			ys := y
			if ys < 0 {
				ys = 0
			}
			ye := y + h
			if ye > H {
				ye = H
			}
			for j := ys; j < ye; j++ {
				pix[j*stride+xr] = col
			}
		}
	}
}

func fillRect(img *image.Gray, x, y, w, h int, col uint8) {
	W := img.Rect.Dx()
	H := img.Rect.Dy()
	if x < 0 {
		w += x
		x = 0
	}
	if y < 0 {
		h += y
		y = 0
	}
	if x+w > W {
		w = W - x
	}
	if y+h > H {
		h = H - y
	}
	if w <= 0 || h <= 0 {
		return
	}
	stride := img.Stride
	pix := img.Pix
	// 首行填充后按行拷贝 (比逐像素快)
	firstOff := y*stride + x
	for i := 0; i < w; i++ {
		pix[firstOff+i] = col
	}
	for j := 1; j < h; j++ {
		off := (y+j)*stride + x
		copy(pix[off:off+w], pix[firstOff:firstOff+w])
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
		if ch >= 'a' && ch <= 'z' {
			ch -= 32
		}
		glyph, ok = basicGlyphs[ch]
		if !ok {
			glyph = basicGlyphs[' ']
		}
	}
	W := img.Rect.Dx()
	H := img.Rect.Dy()
	stride := img.Stride
	pix := img.Pix
	for row, b := range glyph {
		if b == 0 {
			continue
		}
		baseY := y + row*scale
		for colIdx := 0; colIdx < 8; colIdx++ {
			if (b & (0x80 >> colIdx)) == 0 {
				continue
			}
			baseX := x + colIdx*scale
			for sy := 0; sy < scale; sy++ {
				py := baseY + sy
				if py < 0 || py >= H {
					continue
				}
				off := py*stride + baseX
				if baseX < 0 {
					if baseX+scale <= 0 {
						continue
					}
					start := -baseX
					for sx := start; sx < scale; sx++ {
						pix[off+sx] = col
					}
					continue
				}
				if baseX+scale > W {
					end := W - baseX
					if end <= 0 {
						continue
					}
					for sx := 0; sx < end; sx++ {
						pix[off+sx] = col
					}
					continue
				}
				for sx := 0; sx < scale; sx++ {
					pix[off+sx] = col
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
	denom := float64(len(prices) - 1)
	for i := 0; i < len(prices)-1; i++ {
		x0 := x + 2 + int(float64(i)*float64(w-4)/denom)
		y0 := y + 2 + int((maxVal-prices[i])*float64(h-4)/rng)
		x1 := x + 2 + int(float64(i+1)*float64(w-4)/denom)
		y1 := y + 2 + int((maxVal-prices[i+1])*float64(h-4)/rng)
		drawLine(img, x0, y0, x1, y1, 0, 2)
	}
}

func renderScreenImage(data []StockData, width, height int) *image.Gray {
	img := image.NewGray(image.Rect(0, 0, width, height))
	draw.Draw(img, img.Bounds(), &image.Uniform{color.Gray{Y: 255}}, image.Point{}, draw.Src)

	viewMode := GetViewMode()
	effectiveGroup, isAuto := GetEffectiveGroup(viewMode)

	modeTag := fmt.Sprintf("[%s]", viewMode)
	if isAuto {
		modeTag = fmt.Sprintf("[AUTO: %s]", effectiveGroup)
	}
	DrawText(img, 30, 16, "KKANPAN", 32, color.Black)
	DrawText(img, width-460, 20, modeTag, 24, color.Black)

	styleLabel := StyleLabel(GetStyleMode())
	drawRect(img, width-185, 10, 80, 46, 0, 2)
	tw := MeasureText(styleLabel, 24)
	DrawText(img, width-185+(80-tw)/2, 18, styleLabel, 24, color.Black)

	drawRect(img, width-95, 10, 65, 46, 0, 3)
	DrawText(img, width-72, 18, "X", 26, color.Black)

	modes := GetTabModes()
	tabs := make([]struct{ Key, Label string }, len(modes))
	for i, m := range modes {
		tabs[i] = struct{ Key, Label string }{m, m}
	}

	tabCount := len(tabs)
	tabGap := 10
	tabTotalW := width - 60
	tabW := (tabTotalW - (tabCount-1)*tabGap) / tabCount
	tabY := 68
	tabH := 50

	for i, t := range tabs {
		tx := 30 + i*(tabW+tabGap)
		selected := (viewMode == t.Key)
		textW := MeasureText(t.Label, 24)
		padX := (tabW - textW) / 2
		if padX < 2 {
			padX = 2
		}
		if selected {
			fillRect(img, tx, tabY, tabW, tabH, 0)
			DrawText(img, tx+padX, tabY+12, t.Label, 24, color.White)
		} else {
			drawRect(img, tx, tabY, tabW, tabH, 0, 2)
			DrawText(img, tx+padX, tabY+12, t.Label, 24, color.Black)
		}
	}

	drawLine(img, 30, 128, width-30, 128, 0, 3)

	pages := paginate(data, width, height)
	totalPages := len(pages)
	curPage := clampPage(totalPages)
	var curBlocks []block
	if totalPages > 0 && curPage < totalPages {
		curBlocks = pages[curPage]
	}
	startY := 142
	style := GetStyleMode()
	for _, b := range curBlocks {
		if b.isHeader {
			fillRect(img, 30, startY, width-60, 38, 0)
			DrawText(img, 45, startY+8, "[ "+b.group+" ]", 22, color.White)
			startY += b.h
			continue
		}
		item := b.data
		if style == "large" {
			cardH := b.h - b.gap
			drawRect(img, 30, startY, width-60, cardH, 0, 2)
			nameStr := item.Name
			if nameStr == "" {
				nameStr = item.Code
			}
			DrawText(img, 45, startY+18, nameStr, 32, color.Black)
			DrawText(img, 45, startY+62, item.Code, 20, color.Gray{Y: 100})
			arrow := " "
			if item.Change > 0 {
				arrow = "▲"
			} else if item.Change < 0 {
				arrow = "▼"
			}
			priceStr := "--"
			if item.Price > 0 {
				priceStr = fmt.Sprintf("%.2f", item.Price)
			}
			chgStr := fmt.Sprintf("%s %+.2f (%+.2f%%)", arrow, item.Change, item.Pct)
			chartW := width - 490
			chartH := cardH - 30
			if len(item.Prices) > 2 {
				drawSparklineGraph(img, item.Prices, 210, startY+15, chartW, chartH, item.Code)
			}
			priceW := MeasureText(priceStr, 38)
			chgW := MeasureText(chgStr, 24)
			DrawText(img, width-45-priceW, startY+16, priceStr, 38, color.Black)
			DrawText(img, width-45-chgW, startY+64, chgStr, 24, color.Black)
			startY += b.h
		} else {
			cardH := b.h - b.gap
			drawRect(img, 30, startY, width-60, cardH, 0, 2)
			nameStr := item.Name
			if nameStr == "" {
				nameStr = item.Code
			}
			DrawText(img, 45, startY+14, nameStr, 26, color.Black)
			DrawText(img, 45, startY+48, item.Code, 18, color.Gray{Y: 100})
			priceStr, chgStr := stockStrings(item.Price, item.Change, item.Pct, false)
			if len(item.Prices) > 2 {
				drawSparklineGraph(img, item.Prices, 240, startY+12, 480, 70, item.Code)
			}
			priceW := MeasureText(priceStr, 34)
			chgW := MeasureText(chgStr, 20)
			DrawText(img, width-45-priceW, startY+14, priceStr, 34, color.Black)
			DrawText(img, width-45-chgW, startY+52, chgStr, 20, color.Black)
			startY += b.h
		}
	}
	if totalPages > 1 {
		indicator := fmt.Sprintf("%d / %d", curPage+1, totalPages)
		if curPage > 0 {
			indicator = "▲ " + indicator
		}
		if curPage+1 < totalPages {
			indicator = indicator + " ▼"
		}
		iw := MeasureText(indicator, 18)
		DrawText(img, (width-iw)/2, height-40, indicator, 18, color.Gray{Y: 80})
	}

	statusStr := FormatStatusBar()
	drawLine(img, 30, height-58, width-30, height-58, 0, 1)
	DrawText(img, 30, height-24, "Swipe H: switch Tab | Swipe V: flip | Tap [X] exit", 18, color.Gray{Y: 120})
	statusW := MeasureText(statusStr, 18)
	DrawText(img, width-30-statusW, height-24, statusStr, 18, color.Black)
	return img
}

func renderScreenSVG(data []StockData, width, height int) string {
	viewMode := GetViewMode()
	effectiveGroup, isAuto := GetEffectiveGroup(viewMode)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf(`<svg width="%d" height="%d" viewBox="0 0 %d %d" xmlns="http://www.w3.org/2000/svg" shape-rendering="crispEdges">`, width, height, width, height))
	sb.WriteString(`<rect width="100%" height="100%" fill="white"/>`)

	modeTag := fmt.Sprintf("[%s]", viewMode)
	if isAuto {
		modeTag = fmt.Sprintf("[AUTO: %s]", effectiveGroup)
	}
	sb.WriteString(fmt.Sprintf(`<text x="30" y="38" font-family="monospace" font-size="32" font-weight="bold">KKANPAN</text>`))
	sb.WriteString(fmt.Sprintf(`<text x="%d" y="38" font-family="monospace" font-size="24" font-weight="bold" text-anchor="end">%s</text>`, width-270, modeTag))
	sb.WriteString(fmt.Sprintf(`<a href="/style"><rect x="%d" y="10" width="80" height="46" fill="white" stroke="black" stroke-width="2"/><text x="%d" y="34" font-family="monospace" font-size="20" font-weight="bold" text-anchor="middle">%s</text></a>`, width-185, width-145, StyleLabel(GetStyleMode())))
	sb.WriteString(fmt.Sprintf(`<a href="/exit"><rect x="%d" y="10" width="65" height="46" fill="white" stroke="black" stroke-width="3"/><text x="%d" y="34" font-family="monospace" font-size="20" font-weight="bold" text-anchor="middle">X</text></a>`, width-95, width-62))

	modes := GetTabModes()
	tabs := make([]struct{ Key, Label string }, len(modes))
	for i, m := range modes {
		tabs[i] = struct{ Key, Label string }{m, m}
	}
	tabCount := len(tabs)
	tabGap := 10
	tabTotalW := width - 60
	tabW := (tabTotalW - (tabCount-1)*tabGap) / tabCount
	tabY := 68
	tabH := 50

	for i, t := range tabs {
		tx := 30 + i*(tabW+tabGap)
		selected := (viewMode == t.Key)
		if selected {
			sb.WriteString(fmt.Sprintf(`<a href="/switch?view=%s"><rect x="%d" y="%d" width="%d" height="%d" fill="black"/><text x="%d" y="%d" font-family="monospace" font-size="18" fill="white" text-anchor="middle">%s</text></a>`, t.Key, tx, tabY, tabW, tabH, tx+tabW/2, tabY+30, t.Label))
		} else {
			sb.WriteString(fmt.Sprintf(`<a href="/switch?view=%s"><rect x="%d" y="%d" width="%d" height="%d" fill="white" stroke="black" stroke-width="2"/><text x="%d" y="%d" font-family="monospace" font-size="18" fill="black" text-anchor="middle">%s</text></a>`, t.Key, tx, tabY, tabW, tabH, tx+tabW/2, tabY+30, t.Label))
		}
	}

	sb.WriteString(fmt.Sprintf(`<line x1="30" y1="128" x2="%d" y2="128" stroke="black" stroke-width="3"/>`, width-30))

	pages := paginate(data, width, height)
	totalPages := len(pages)
	curPage := clampPage(totalPages)
	var curBlocks []block
	if totalPages > 0 && curPage < totalPages {
		curBlocks = pages[curPage]
	}
	style := GetStyleMode()
	startY := 142
	for _, b := range curBlocks {
		if b.isHeader {
			sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="38" fill="black"/>`, startY, width-60))
			sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="22" fill="white">[ %s ]</text>`, startY+24, b.group))
			startY += b.h
			continue
		}
		cardH := b.h - b.gap
		if style == "large" {
			// 大卡 SVG: 放大字体+大图
			nameStr := b.data.Name
			if nameStr == "" {
				nameStr = b.data.Code
			}
			sb.WriteString(fmt.Sprintf(`<rect x="30" y="%d" width="%d" height="%d" fill="none" stroke="black" stroke-width="2"/>`, startY, width-60, cardH))
			sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="28">%s</text>`, startY+30, nameStr))
			sb.WriteString(fmt.Sprintf(`<text x="45" y="%d" font-family="monospace" font-size="16" fill="#666">%s</text>`, startY+52, b.data.Code))
			priceStr, chgStr := stockStrings(b.data.Price, b.data.Change, b.data.Pct, true)
			if len(b.data.Prices) > 2 {
				pts, _, _, _ := sparklinePoints(b.data.Prices, b.data.Code, width-490, cardH-30)
				gx, gy := 210, startY+15
				shifted := make([]string, 0, len(pts))
				for _, pt := range pts {
					parts := strings.Split(pt, ",")
					if len(parts) == 2 {
						xv, _ := strconv.ParseFloat(parts[0], 64)
						yv, _ := strconv.ParseFloat(parts[1], 64)
						shifted = append(shifted, fmt.Sprintf("%.1f,%.1f", xv+float64(gx), yv+float64(gy)))
					}
				}
				sb.WriteString(fmt.Sprintf(`<rect x="%d" y="%d" width="%d" height="%d" fill="none" stroke="black" stroke-width="1"/>`, gx, gy, width-490, cardH-30))
				sb.WriteString(fmt.Sprintf(`<polyline fill="none" stroke="black" stroke-width="1.5" points="%s"/>`, strings.Join(shifted, " ")))
			}
			sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="28" font-weight="bold" text-anchor="end">%s</text>`, width-45, startY+32, priceStr))
			sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="18" text-anchor="end">%s</text>`, width-45, startY+58, chgStr))
		} else {
			svgWriteCard(&sb, b.data, startY, width, cardH, true)
		}
		startY += b.h
	}

	if totalPages > 1 {
		indicator := fmt.Sprintf("%d / %d", curPage+1, totalPages)
		if curPage > 0 {
			indicator = "▲ " + indicator
		}
		if curPage+1 < totalPages {
			indicator = indicator + " ▼"
		}
		sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="18" fill="#555" text-anchor="middle">%s</text>`, width/2, height-40, indicator))
	}

	statusStr := FormatStatusBar()
	sb.WriteString(fmt.Sprintf(`<line x1="30" y1="%d" x2="%d" y2="%d" stroke="black" stroke-width="1"/>`, height-58, width-30, height-58))
	sb.WriteString(fmt.Sprintf(`<text x="30" y="%d" font-family="monospace" font-size="18" fill="#888">Swipe V: flip | Tap tabs | Tap [X] exit</text>`, height-16))
	sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="18" font-weight="bold" text-anchor="end">%s</text>`, width-30, height-16, statusStr))
	sb.WriteString(`</svg>`)
	return sb.String()
}
