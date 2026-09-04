package main

import (
	"fmt"
	"image"
	"image/color"
	"math"
	"strconv"
	"strings"
)

var globalCanvas *image.Gray // 复用渲染画布, 避免每次 NewGray 分配 ~1.5MB

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

func drawSparklineGraph(img *image.Gray, item StockData, x, y, w, h int) {
	prices := item.Prices
	if len(prices) < 2 {
		return
	}
	drawRect(img, x, y, w, h, 0, 1)
	_, minVal, maxVal, rng := sparklinePoints(prices, item.Code, w, h)

	isYahoo := len(item.Timestamps) == len(prices) && item.RegularEnd > item.RegularStart

	// 昨收基准价: Yahoo用chartPrevClose, 腾讯用Prev
	refVal := item.Prev
	if isYahoo {
		refVal = item.ChartPrevClose
	}
	if refVal == 0 {
		refVal = (minVal + maxVal) / 2
	}

	isLarge := GetStyleMode() == "large"
	if isLarge {
		// large: 扩展min/max包含参考价, 虚线位置比例正确
		if refVal < minVal {
			minVal = refVal
		}
		if refVal > maxVal {
			maxVal = refVal
		}
	} else {
		// normal: 超出当日范围就不画
		if refVal < minVal || refVal > maxVal {
			refVal = 0
		}
	}
	rng = maxVal - minVal
	if rng == 0 {
		rng = 1
	}

	// 昨收基准虚线
	if refVal > 0 {
		refY := y + 2 + int((maxVal-refVal)*float64(h-4)/rng)
		for lx := x + 4; lx < x+w-4; lx += 6 {
			if lx+3 < x+w-4 && refY >= 0 && refY < img.Rect.Dy() {
				img.SetGray(lx, refY, color.Gray{Y: 128})
				img.SetGray(lx+1, refY, color.Gray{Y: 128})
			}
		}
	}

	// 实线绘制
	for i := 0; i < len(prices)-1; i++ {
		var x0, x1 int
		if isYahoo {
			totalSec := float64(item.RegularEnd - item.RegularStart)
			x0 = x + 2 + int(float64(item.Timestamps[i]-item.RegularStart)*float64(w-4)/totalSec)
			x1 = x + 2 + int(float64(item.Timestamps[i+1]-item.RegularStart)*float64(w-4)/totalSec)
		} else {
			total := chartTotal(item.Code, len(prices))
			x0 = x + 2 + int(float64(i)*float64(w-4)/float64(total))
			x1 = x + 2 + int(float64(i+1)*float64(w-4)/float64(total))
		}
		y0 := y + 2 + int((maxVal-prices[i])*float64(h-4)/rng)
		y1 := y + 2 + int((maxVal-prices[i+1])*float64(h-4)/rng)
		drawLine(img, x0, y0, x1, y1, 0, 2)
	}
}

func renderScreenImage(data []StockData, width, height int) *image.Gray {
	// 复用全局画布, 避免每次渲染分配 ~1.5MB 并触发 GC
	if globalCanvas == nil || globalCanvas.Rect.Dx() != width || globalCanvas.Rect.Dy() != height {
		globalCanvas = image.NewGray(image.Rect(0, 0, width, height))
	}
	img := globalCanvas
	// memset 白色 (Y=255), 比 draw.Draw + Uniform 快
	for i := range img.Pix {
		img.Pix[i] = 255
	}

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
	pl := ComputePageLayout(curBlocks, width, height)
	style := GetStyleMode()
	for _, bl := range pl.Blocks {
		if bl.IsHeader {
			fillRect(img, bl.Bar.X, bl.Bar.Y, bl.Bar.W, bl.Bar.H, 0)
			DrawText(img, bl.BarText.X, bl.BarText.Y, "[ "+bl.Group+" ]", bl.BarText.Size, color.White)
			continue
		}
		item := bl.Data
		drawRect(img, bl.Card.X, bl.Card.Y, bl.Card.W, bl.Card.H, 0, cardBorder)
		nameStr := item.Name
		if nameStr == "" {
			nameStr = item.Code
		}
		DrawText(img, bl.Name.X, bl.Name.Y, nameStr, bl.Name.Size, color.Black)
		DrawText(img, bl.Code.X, bl.Code.Y, item.Code, bl.Code.Size, color.Gray{Y: 100})
		if style == "large" {
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
			if len(item.Prices) > 2 {
				drawSparklineGraph(img, item, bl.Spark.X, bl.Spark.Y, bl.Spark.W, bl.Spark.H)
			}
			priceW := MeasureText(priceStr, bl.Price.Size)
			chgW := MeasureText(chgStr, bl.Chg.Size)
			DrawText(img, bl.Price.X-priceW, bl.Price.Y, priceStr, bl.Price.Size, color.Black)
			DrawText(img, bl.Chg.X-chgW, bl.Chg.Y, chgStr, bl.Chg.Size, color.Black)
		} else {
			priceStr, chgStr := stockStrings(item.Price, item.Change, item.Pct, false)
			if len(item.Prices) > 2 {
				drawSparklineGraph(img, item, bl.Spark.X, bl.Spark.Y, bl.Spark.W, bl.Spark.H)
			}
			priceW := MeasureText(priceStr, bl.Price.Size)
			chgW := MeasureText(chgStr, bl.Chg.Size)
			DrawText(img, bl.Price.X-priceW, bl.Price.Y, priceStr, bl.Price.Size, color.Black)
			DrawText(img, bl.Chg.X-chgW, bl.Chg.Y, chgStr, bl.Chg.Size, color.Black)
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
	pl := ComputePageLayout(curBlocks, width, height)
	style := GetStyleMode()
	for _, bl := range pl.Blocks {
		if bl.IsHeader {
			sb.WriteString(fmt.Sprintf(`<rect x="%d" y="%d" width="%d" height="%d" fill="black"/>`, bl.Bar.X, bl.Bar.Y, bl.Bar.W, bl.Bar.H))
			sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="%d" fill="white">[ %s ]</text>`, bl.BarText.X, bl.BarText.Y+16, bl.BarText.Size, bl.Group))
			continue
		}
		item := bl.Data
		nameStr := item.Name
		if nameStr == "" {
			nameStr = item.Code
		}
		sb.WriteString(fmt.Sprintf(`<rect x="%d" y="%d" width="%d" height="%d" fill="none" stroke="black" stroke-width="%d"/>`, bl.Card.X, bl.Card.Y, bl.Card.W, bl.Card.H, cardBorder))
		sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="%d">%s</text>`, bl.Name.X, bl.Name.Y+bl.Name.Size, bl.Name.Size, nameStr))
		sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="%d" fill="#666">%s</text>`, bl.Code.X, bl.Code.Y+bl.Code.Size, bl.Code.Size, item.Code))
		priceStr, chgStr := stockStrings(item.Price, item.Change, item.Pct, true)
		if len(item.Prices) > 2 {
			gx, gy, sparkW, sparkH := bl.Spark.X, bl.Spark.Y, bl.Spark.W, bl.Spark.H
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
			isYahoo := len(item.Timestamps) == len(prices) && item.RegularEnd > item.RegularStart
			refVal := item.Prev
			if isYahoo {
				refVal = item.ChartPrevClose
			}
			if refVal == 0 {
				refVal = (minVal + maxVal) / 2
			}
			if style == "large" {
				if refVal < minVal {
					minVal = refVal
				}
				if refVal > maxVal {
					maxVal = refVal
				}
			} else {
				if refVal < minVal || refVal > maxVal {
					refVal = 0
				}
			}
			rng := maxVal - minVal
			if rng == 0 {
				rng = 1
			}
			if refVal > 0 {
				refY := float64(gy) + 2.0 + (maxVal-refVal)*float64(sparkH-4)/rng
				sb.WriteString(fmt.Sprintf(`<line x1="%d" y1="%.1f" x2="%d" y2="%.1f" stroke="black" stroke-width="1" stroke-dasharray="4,4"/>`, gx+4, refY, gx+sparkW-4, refY))
			}
			var totalSec float64
			if isYahoo {
				totalSec = float64(item.RegularEnd - item.RegularStart)
			}
			total := chartTotal(item.Code, len(prices))
			pts := make([]string, 0, len(prices))
			for i, p := range prices {
				var x float64
				if isYahoo {
					x = float64(gx) + 2.0 + float64(item.Timestamps[i]-item.RegularStart)*float64(sparkW-4)/totalSec
				} else {
					x = float64(gx) + 2.0 + float64(i)*float64(sparkW-4)/float64(total)
				}
				y := float64(gy) + 2.0 + (maxVal-p)*float64(sparkH-4)/rng
				pts = append(pts, strconv.FormatFloat(x, 'f', 1, 64)+","+strconv.FormatFloat(y, 'f', 1, 64))
			}
			sb.WriteString(fmt.Sprintf(`<rect x="%d" y="%d" width="%d" height="%d" fill="none" stroke="black" stroke-width="1"/>`, gx, gy, sparkW, sparkH))
			sb.WriteString(fmt.Sprintf(`<polyline fill="none" stroke="black" stroke-width="1.5" points="%s"/>`, strings.Join(pts, " ")))
		}
		sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="%d" font-weight="bold" text-anchor="end">%s</text>`, bl.Price.X, bl.Price.Y+bl.Price.Size, bl.Price.Size, priceStr))
		sb.WriteString(fmt.Sprintf(`<text x="%d" y="%d" font-family="monospace" font-size="%d" text-anchor="end">%s</text>`, bl.Chg.X, bl.Chg.Y+bl.Chg.Size, bl.Chg.Size, chgStr))
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
