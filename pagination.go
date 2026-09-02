package main

import "sync"

var (
	pageMu      sync.RWMutex
	currentPage int
)

// pagination helpers — KISS: 按可用高度切块, 超出即新页

type block struct {
	isHeader bool
	group    string
	data     StockData
	h        int // 高度含 gap
	gap      int
}

func pageHeight(height int) int {
	// startY 142 到 height-100 之间的可用高度 (与 render.go 一致)
	h := height - 142 - 100
	if h < 200 {
		h = 200
	}
	return h
}

func buildBlocks(data []StockData, effectiveGroup string, isAuto bool) []block {
	groups := make(map[string][]StockData)
	for _, d := range data {
		groups[d.Group] = append(groups[d.Group], d)
	}
	var blocks []block
	if effectiveGroup == "" {
		if !isAuto || len(appConfig.PinnedGroups) == 0 {
			return nil // 真正空屏
		}
		// AUTO 空档期仅显示常驻组
		for _, pg := range appConfig.PinnedGroups {
			futs, ok := groups[pg]
			if !ok || len(futs) == 0 {
				continue
			}
			blocks = append(blocks, block{isHeader: true, group: pg, h: 44, gap: 6})
			for _, f := range futs {
				blocks = append(blocks, block{isHeader: false, group: pg, data: f, h: 93, gap: 8})
			}
		}
		return blocks
	}
	if effectiveGroup != "ALL" && len(groups[effectiveGroup]) > 0 {
		list := groups[effectiveGroup]
		blocks = append(blocks, block{isHeader: true, group: effectiveGroup, h: 52, gap: 8}) // 44+8
		cardH := 145
		if len(list) <= 3 {
			cardH = 175
		} else if len(list) > 6 {
			cardH = 110
		}
		for _, it := range list {
			blocks = append(blocks, block{isHeader: false, group: effectiveGroup, data: it, h: cardH + 10, gap: 10})
		}
		if isAuto {
			for _, pg := range appConfig.PinnedGroups {
				if pg == effectiveGroup {
					continue
				}
				futs, ok := groups[pg]
				if !ok || len(futs) == 0 {
					continue
				}
				blocks = append(blocks, block{isHeader: true, group: pg, h: 44, gap: 6}) // 38+6
				for _, f := range futs {
					blocks = append(blocks, block{isHeader: false, group: pg, data: f, h: 93, gap: 8}) // 85+8
				}
			}
		}
	} else if effectiveGroup == "ALL" {
		for _, gName := range GetAllGroups() {
			list, ok := groups[gName]
			if !ok || len(list) == 0 {
				continue
			}
			blocks = append(blocks, block{isHeader: true, group: gName, h: 44, gap: 6})
			for _, it := range list {
				blocks = append(blocks, block{isHeader: false, group: gName, data: it, h: 103, gap: 8}) // 95+8
			}
		}
	}
	return blocks
}

func paginate(data []StockData, width, height int) [][]block {
	// width 未使用, 保留以对齐 render 签名, 未来可按宽度分栏
	_ = width
	viewMode := GetViewMode()
	eff, isAuto := GetEffectiveGroup(viewMode)
	blocks := buildBlocks(data, eff, isAuto)
	ph := pageHeight(height)
	var pages [][]block
	var cur []block
	curH := 0
	for i, b := range blocks {
		// 避免 header 孤悬在页尾 (header 是该页最后一块且下一块是同组 card)
		if b.isHeader && curH+b.h > ph && curH > 0 {
			pages = append(pages, cur)
			cur = nil
			curH = 0
		}
		if curH+b.h > ph && curH > 0 {
			// header 孤悬检查: 若当前块是 card 且上一块是 header 且 header 单独占页尾
			// 已在上一分支处理, 这里仅切页
			pages = append(pages, cur)
			cur = nil
			curH = 0
			// 跨页续组无需重复 header, 直接续 card
			_ = i
		}
		cur = append(cur, b)
		curH += b.h
	}
	if len(cur) > 0 {
		pages = append(pages, cur)
	}
	if len(pages) == 0 {
		pages = [][]block{{}}
	}
	return pages
}

func GetTotalPages(data []StockData, width, height int) int {
	return len(paginate(data, width, height))
}

func GetCurrentPage() int {
	pageMu.RLock()
	defer pageMu.RUnlock()
	return currentPage
}

func SetCurrentPage(p int) {
	pageMu.Lock()
	currentPage = p
	pageMu.Unlock()
}

func ResetPage() {
	pageMu.Lock()
	currentPage = 0
	pageMu.Unlock()
}

func NextPage(total int) bool {
	pageMu.Lock()
	defer pageMu.Unlock()
	if currentPage+1 < total {
		currentPage++
		return true
	}
	return false
}

func PrevPage() bool {
	pageMu.Lock()
	defer pageMu.Unlock()
	if currentPage > 0 {
		currentPage--
		return true
	}
	return false
}

// clampPage 保证 currentPage 在 [0,total-1]
func clampPage(total int) int {
	pageMu.Lock()
	defer pageMu.Unlock()
	if total <= 0 {
		currentPage = 0
		return 0
	}
	if currentPage >= total {
		currentPage = total - 1
	}
	if currentPage < 0 {
		currentPage = 0
	}
	return currentPage
}

// neededStocksData 仅用于手势时估算总页数 (无缓存时用配置构造 dummy StockData)
func neededStocksData() []StockData {
	cfgs := neededStocks()
	out := make([]StockData, len(cfgs))
	for i, c := range cfgs {
		out[i] = StockData{Code: c.Code, Name: c.Name, Group: c.Group}
	}
	return out
}
