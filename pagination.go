package main

import "sync"

var (
	pageMu      sync.RWMutex
	currentPage int
)

type block struct {
	isHeader bool
	group    string
	data     StockData
	h        int
	gap      int
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
	style := GetStyleMode()
	var blocks []block
	if effectiveGroup == "ALL" {
		for _, gName := range GetAllGroups() {
			list := groups[gName]
			if len(list) == 0 {
				continue
			}
			if style == "large" {
				blocks = append(blocks, block{isHeader: true, group: gName, h: 44, gap: 6})
				for _, it := range list {
					blocks = append(blocks, block{isHeader: false, group: gName, data: it, h: 155, gap: 10})
				}
			} else {
				blocks = appendGroup(blocks, gName, list, 103)
			}
		}
		return blocks
	}
	if isAuto {
		matching := GetMatchingAutoGroups()
		if len(matching) == 0 {
			return nil
		}
		for _, g := range matching {
			list := groups[g]
			if len(list) == 0 {
				continue
			}
			if style == "large" {
				blocks = append(blocks, block{isHeader: true, group: g, h: 44, gap: 6})
				for _, it := range list {
					blocks = append(blocks, block{isHeader: false, group: g, data: it, h: 155, gap: 10})
				}
			} else {
				blocks = appendGroup(blocks, g, list, 103)
			}
		}
		return blocks
	}
	list := groups[effectiveGroup]
	if len(list) == 0 {
		return blocks
	}
	if style == "large" {
		blocks = append(blocks, block{isHeader: true, group: effectiveGroup, h: 44, gap: 6})
		for _, it := range list {
			blocks = append(blocks, block{isHeader: false, group: effectiveGroup, data: it, h: 155, gap: 10})
		}
	} else {
		blocks = appendGroup(blocks, effectiveGroup, list, 103)
	}
	return blocks
}

func paginate(data []StockData, width, height int) [][]block {
	_ = width
	eff, isAuto := GetEffectiveGroup(GetViewMode())
	blocks := buildBlocks(data, eff, isAuto)
	ph := height - 142 - 70
	if ph < 200 {
		ph = 200
	}
	var pages [][]block
	var cur []block
	curH := 0
	for i, b := range blocks {
		if b.isHeader && curH+b.h-b.gap > ph && curH > 0 {
			pages = append(pages, cur)
			cur = nil
			curH = 0
		}
		if curH+b.h-b.gap > ph && curH > 0 {
			pages = append(pages, cur)
			cur = nil
			curH = 0
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
