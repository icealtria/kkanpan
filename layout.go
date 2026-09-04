package main

const (
	marginX       = 30
	contentTop    = 142
	bottomReserve = 70
	dividerY      = 128

	headerH     = 48
	headerBarH  = 38
	headerGap   = 4
	normalCardH = 103
	largeCardH  = 155
	cardBorder  = 1

	titleX    = 30
	titleY    = 16
	titleSize = 32

	modeTagX    = -460
	modeTagY    = 20
	modeTagSize = 24

	tabBarY  = 68
	tabBarH  = 50
	tabGap   = 10
	tabTextY = 12
	tabTextSz = 24

	styleBtnX = -185
	styleBtnY = 10
	styleBtnW = 80
	styleBtnH = 46

	exitBtnX = -95
	exitBtnY = 10
	exitBtnW = 65
	exitBtnH = 46

	pageIndicatorY = -40
	statusLineY    = -58
	statusTextY    = -24
	statusTextSz   = 18

	normalNameY  = 14
	normalNameSz = 26
	normalCodeY  = 48
	normalCodeSz = 18
	normalSparkX = 240
	normalSparkY = 20
	normalSparkW = 480
	normalSparkH = 63
	normalPriceY = 14
	normalPriceSz = 34
	normalChgY   = 52
	normalChgSz  = 20

	largeNameY   = 18
	largeNameSz  = 32
	largeCodeY   = 62
	largeCodeSz  = 20
	largeSparkX  = 210
	largeSparkY  = 15
	largeSparkWOfs = 490
	largePriceY  = 16
	largePriceSz = 38
	largeChgY    = 64
	largeChgSz   = 24
)

type Rect struct{ X, Y, W, H int }

type TextLabel struct {
	X, Y, Size int
}

type BlockLayout struct {
	IsHeader bool
	Group    string
	Data     StockData
	H        int
	Outer    Rect
	Bar      Rect
	BarText  TextLabel
	Card     Rect
	Name     TextLabel
	Code     TextLabel
	Spark    Rect
	Price    TextLabel
	Chg      TextLabel
}

type PageLayout struct {
	W, H   int
	Blocks []BlockLayout
}

func ComputePageLayout(blocks []block, width, height int) PageLayout {
	style := GetStyleMode()
	pl := PageLayout{W: width, H: height, Blocks: make([]BlockLayout, len(blocks))}
	y := contentTop
	for i, b := range blocks {
		if b.isHeader {
			pl.Blocks[i] = layoutHeader(b, y, width)
		} else {
			pl.Blocks[i] = layoutCard(b, y, width, style)
		}
		y += b.h
	}
	return pl
}

func layoutHeader(b block, startY, width int) BlockLayout {
	bl := BlockLayout{IsHeader: true, Group: b.group, H: b.h}
	bl.Outer = Rect{X: marginX, Y: startY, W: width - 2*marginX, H: b.h}
	bl.Bar = Rect{X: marginX, Y: startY + headerGap, W: width - 2*marginX, H: headerBarH}
	bl.BarText = TextLabel{X: marginX + 15, Y: startY + headerGap + 8, Size: 22}
	return bl
}

func layoutCard(b block, startY, width int, style string) BlockLayout {
	bl := BlockLayout{IsHeader: false, Data: b.data, H: b.h}
 cardW := width - 2*marginX
	bl.Outer = Rect{X: marginX, Y: startY, W: cardW, H: b.h}
	bl.Card = Rect{X: marginX, Y: startY, W: cardW, H: b.h}

	if style == "large" {
		bl.Name = TextLabel{X: marginX + 15, Y: startY + largeNameY, Size: largeNameSz}
		bl.Code = TextLabel{X: marginX + 15, Y: startY + largeCodeY, Size: largeCodeSz}
		bl.Spark = Rect{X: largeSparkX, Y: startY + largeSparkY, W: width - largeSparkWOfs, H: b.h - 30}
		bl.Price = TextLabel{X: width - 45, Y: startY + largePriceY, Size: largePriceSz}
		bl.Chg = TextLabel{X: width - 45, Y: startY + largeChgY, Size: largeChgSz}
	} else {
		bl.Name = TextLabel{X: marginX + 15, Y: startY + normalNameY, Size: normalNameSz}
		bl.Code = TextLabel{X: marginX + 15, Y: startY + normalCodeY, Size: normalCodeSz}
		bl.Spark = Rect{X: normalSparkX, Y: startY + normalSparkY, W: normalSparkW, H: normalSparkH}
		bl.Price = TextLabel{X: width - 45, Y: startY + normalPriceY, Size: normalPriceSz}
		bl.Chg = TextLabel{X: width - 45, Y: startY + normalChgY, Size: normalChgSz}
	}
	return bl
}

type TabInfo struct {
	Key, Label string
	X, Y, W, H int
}

func ComputeTabLayout(width int) []TabInfo {
	modes := GetTabModes()
	n := len(modes)
	if n == 0 {
		return nil
	}
	totalW := width - 2*marginX
	w := (totalW - (n-1)*tabGap) / n
	tabs := make([]TabInfo, n)
	for i, m := range modes {
		tabs[i] = TabInfo{
			Key: m, Label: m,
			X: marginX + i*(w+tabGap), Y: tabBarY,
			W: w, H: tabBarH,
		}
	}
	return tabs
}
