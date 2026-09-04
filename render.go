package main

import (
	"fmt"
	"image"
	"image/color"
	"math"
	"strconv"
)

var globalCanvas *image.Gray

func sparklinePoints(prices []float64, w, h int) (pts []string, minVal, maxVal, rng float64) {
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

func stockStrings(price, change, pct float64) (priceStr, chgStr string) {
	if price > 0 {
		priceStr = fmt.Sprintf("%.2f", price)
	} else {
		priceStr = "--"
	}
	arrow := " "
	if change > 0 {
		arrow = "▲"
	} else if change < 0 {
		arrow = "▼"
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
	for bx := range border {
		yt := y + bx
		yb := y + h - 1 - bx
		if yt >= 0 && yt < H {
			off := yt * stride
			xs := max(x, 0)
			xe := min(x+w, W)
			for i := xs; i < xe; i++ {
				pix[off+i] = col
			}
		}
		if yb >= 0 && yb < H && yb != yt {
			off := yb * stride
			xs := max(x, 0)
			xe := min(x+w, W)
			for i := xs; i < xe; i++ {
				pix[off+i] = col
			}
		}
		xt := x + bx
		xr := x + w - 1 - bx
		if xt >= 0 && xt < W {
			ys := max(y, 0)
			ye := min(y+h, H)
			for j := ys; j < ye; j++ {
				pix[j*stride+xt] = col
			}
		}
		if xr >= 0 && xr < W && xr != xt {
			ys := max(y, 0)
			ye := min(y+h, H)
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
		for colIdx := range 8 {
			if (b & (0x80 >> colIdx)) == 0 {
				continue
			}
			baseX := x + colIdx*scale
			for sy := range scale {
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
					for sx := range end {
						pix[off+sx] = col
					}
					continue
				}
				for sx := range scale {
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
	_, minVal, maxVal, rng := sparklinePoints(prices, w, h)

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
	if globalCanvas == nil || globalCanvas.Rect.Dx() != width || globalCanvas.Rect.Dy() != height {
		globalCanvas = image.NewGray(image.Rect(0, 0, width, height))
	}
	img := globalCanvas
	for i := range img.Pix {
		img.Pix[i] = 255
	}

	viewMode := GetViewMode()
	effectiveGroup, isAuto := GetEffectiveGroup(viewMode)

	// ── Header: title + mode tag + buttons ──
	modeTag := fmt.Sprintf("[%s]", viewMode)
	if isAuto {
		modeTag = fmt.Sprintf("[AUTO: %s]", effectiveGroup)
	}
	DrawText(img, titleX, titleY, "KKANPAN", titleSize, color.Black)
	DrawText(img, width+modeTagX, modeTagY, modeTag, modeTagSize, color.Black)

	styleLabel := StyleLabel(GetStyleMode())
	drawRect(img, width+styleBtnX, styleBtnY, styleBtnW, styleBtnH, 0, 2)
	tw := MeasureText(styleLabel, tabTextSz)
	DrawText(img, width+styleBtnX+(styleBtnW-tw)/2, styleBtnY+8, styleLabel, tabTextSz, color.Black)

	drawRect(img, width+exitBtnX, exitBtnY, exitBtnW, exitBtnH, 0, 3)
	DrawText(img, width+exitBtnX+exitBtnW/2-exitBtnH/2+3, exitBtnY+8, "X", 26, color.Black)

	// ── Tab bar ──
	for _, t := range ComputeTabLayout(width) {
		textW := MeasureText(t.Label, tabTextSz)
		padX := max((t.W-textW)/2, 2)
		if viewMode == t.Key {
			fillRect(img, t.X, t.Y, t.W, t.H, 0)
			DrawText(img, t.X+padX, t.Y+tabTextY, t.Label, tabTextSz, color.White)
		} else {
			drawRect(img, t.X, t.Y, t.W, t.H, 0, 2)
			DrawText(img, t.X+padX, t.Y+tabTextY, t.Label, tabTextSz, color.Black)
		}
	}

	drawLine(img, marginX, dividerY, width-marginX, dividerY, 0, 3)

	// ── Content area: stock cards ──
	pages := paginate(data, height)
	totalPages := len(pages)
	curPage := clampPage(totalPages)
	var curBlocks []block
	if totalPages > 0 && curPage < totalPages {
		curBlocks = pages[curPage]
	}
	for _, bl := range ComputePageLayout(curBlocks, width, height).Blocks {
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
		if len(item.Prices) > 2 {
			drawSparklineGraph(img, item, bl.Spark.X, bl.Spark.Y, bl.Spark.W, bl.Spark.H)
		}
		priceStr, chgStr := stockStrings(item.Price, item.Change, item.Pct)
		priceW := MeasureText(priceStr, bl.Price.Size)
		chgW := MeasureText(chgStr, bl.Chg.Size)
		DrawText(img, bl.Price.X-priceW, bl.Price.Y, priceStr, bl.Price.Size, color.Black)
		DrawText(img, bl.Chg.X-chgW, bl.Chg.Y, chgStr, bl.Chg.Size, color.Black)
	}

	// ── Footer: page indicator + status bar ──
	if totalPages > 1 {
		indicator := fmt.Sprintf("%d / %d", curPage+1, totalPages)
		if curPage > 0 {
			indicator = "▲ " + indicator
		}
		if curPage+1 < totalPages {
			indicator = indicator + " ▼"
		}
		iw := MeasureText(indicator, statusTextSz)
		DrawText(img, (width-iw)/2, height+pageIndicatorY, indicator, statusTextSz, color.Gray{Y: 80})
	}

	statusStr := FormatStatusBar()
	drawLine(img, marginX, height+statusLineY, width-marginX, height+statusLineY, 0, 1)
	DrawText(img, marginX, height+statusTextY, "Swipe H: switch Tab | Swipe V: flip | Tap [X] exit", statusTextSz, color.Gray{Y: 120})
	statusW := MeasureText(statusStr, statusTextSz)
	DrawText(img, width-marginX-statusW, height+statusTextY, statusStr, statusTextSz, color.Black)
	return img
}
