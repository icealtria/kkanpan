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
}

func appendGroup(blocks []block, group string, list []StockData, cardH int) []block {
	if len(list) == 0 {
		return blocks
	}
	blocks = append(blocks, block{isHeader: true, group: group, h: headerH})
	for _, it := range list {
		blocks = append(blocks, block{isHeader: false, group: group, data: it, h: cardH})
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
				blocks = append(blocks, block{isHeader: true, group: gName, h: headerH})
				for _, it := range list {
					blocks = append(blocks, block{isHeader: false, group: gName, data: it, h: largeCardH})
				}
			} else {
				blocks = appendGroup(blocks, gName, list, normalCardH)
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
				blocks = append(blocks, block{isHeader: true, group: g, h: headerH})
				for _, it := range list {
					blocks = append(blocks, block{isHeader: false, group: g, data: it, h: largeCardH})
				}
			} else {
				blocks = appendGroup(blocks, g, list, normalCardH)
			}
		}
		return blocks
	}
	list := groups[effectiveGroup]
	if len(list) == 0 {
		return blocks
	}
	if style == "large" {
		blocks = append(blocks, block{isHeader: true, group: effectiveGroup, h: headerH})
		for _, it := range list {
			blocks = append(blocks, block{isHeader: false, group: effectiveGroup, data: it, h: largeCardH})
		}
	} else {
		blocks = appendGroup(blocks, effectiveGroup, list, normalCardH)
	}
	return blocks
}

func paginate(data []StockData, height int) [][]block {
	eff, isAuto := GetEffectiveGroup(GetViewMode())
	blocks := buildBlocks(data, eff, isAuto)
	ph := max(height-contentTop-bottomReserve, 200)
	var pages [][]block
	var cur []block
	curH := 0
	for _, b := range blocks {
		if curH+b.h > ph && curH > 0 {
			pages = append(pages, cur)
			cur = nil
			curH = 0
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

func GetTotalPages(data []StockData, height int) int {
	return len(paginate(data, height))
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
