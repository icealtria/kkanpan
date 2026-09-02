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

func appendGroup(blocks []block, group string, list []StockData, cardH int) []block {
	if len(list) == 0 {
		return blocks
	}
	blocks = append(blocks, block{isHeader: true, group: group, h: 44, gap: 6})
	for _, it := range list {
		blocks = append(blocks, block{isHeader: false, group: group, data: it, h: cardH, gap: 8})
	}
	return blocks
}

func buildBlocks(data []StockData, effectiveGroup string, isAuto bool) []block {
	groups := make(map[string][]StockData)
	for _, d := range data {
		groups[d.Group] = append(groups[d.Group], d)
	}
	var blocks []block
	if effectiveGroup == "" {
		if !isAuto || len(appConfig.PinnedGroups) == 0 {
			return nil
		}
		for _, pg := range appConfig.PinnedGroups {
			blocks = appendGroup(blocks, pg, groups[pg], 103)
		}
		return blocks
	}
	if effectiveGroup != "ALL" && len(groups[effectiveGroup]) > 0 {
		blocks = appendGroup(blocks, effectiveGroup, groups[effectiveGroup], 103)
		if isAuto {
			for _, pg := range appConfig.PinnedGroups {
				if pg == effectiveGroup {
					continue
				}
				blocks = appendGroup(blocks, pg, groups[pg], 93)
			}
		}
	} else if effectiveGroup == "ALL" {
		for _, gName := range GetAllGroups() {
			blocks = appendGroup(blocks, gName, groups[gName], 103)
		}
	}
	return blocks
}

func paginate(data []StockData, width, height int) [][]block {
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

func neededStocksData() []StockData {
	cfgs := neededStocks()
	out := make([]StockData, len(cfgs))
	for i, c := range cfgs {
		out[i] = StockData{Code: c.Code, Name: c.Name, Group: c.Group}
	}
	return out
}
