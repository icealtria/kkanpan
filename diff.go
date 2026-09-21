package main

import (
	"bytes"
	"image"
	"log"
	"sync"
)

type DirtyRect struct {
	X, Y, W, H int
}

type ScreenDiffer struct {
	mu        sync.Mutex
	prevFrame *image.Gray
	blockSize int
}

var screenDiffer = &ScreenDiffer{
	blockSize: 8,
}

func (sd *ScreenDiffer) FindDirtyRects(oldImg, newImg *image.Gray) []DirtyRect {
	if oldImg == nil {
		return []DirtyRect{{0, 0, newImg.Rect.Dx(), newImg.Rect.Dy()}}
	}

	w, h := newImg.Rect.Dx(), newImg.Rect.Dy()
	ow, oh := oldImg.Rect.Dx(), oldImg.Rect.Dy()
	if w != ow || h != oh {
		return []DirtyRect{{0, 0, w, h}}
	}

	bs := sd.blockSize
	cols := (w + bs - 1) / bs
	rows := (h + bs - 1) / bs

	dirty := make([]bool, cols*rows)
	hasDirty := false

	for by := range rows {
		for bx := range cols {
			if sd.isBlockDirty(oldImg, newImg, bx*bs, by*bs, bs, w, h) {
				dirty[by*cols+bx] = true
				hasDirty = true
			}
		}
	}

	if !hasDirty {
		return nil
	}
	return mergeBlocks(dirty, cols, rows, bs, w, h)
}

func (sd *ScreenDiffer) isBlockDirty(oldImg, newImg *image.Gray, x0, y0, bs, imgW, imgH int) bool {
	for y := y0; y < y0+bs && y < imgH; y++ {
		rowStart := y * oldImg.Stride
		colStart := rowStart + x0
		colEnd := min(rowStart+x0+bs, rowStart+imgW)
		if colStart >= len(oldImg.Pix) || colStart >= len(newImg.Pix) {
			continue
		}
		if colEnd > len(oldImg.Pix) {
			colEnd = len(oldImg.Pix)
		}
		if colEnd > len(newImg.Pix) {
			colEnd = len(newImg.Pix)
		}
		if !bytes.Equal(oldImg.Pix[colStart:colEnd], newImg.Pix[colStart:colEnd]) {
			return true
		}
	}
	return false
}

func mergeBlocks(dirty []bool, cols, rows, bs, imgW, imgH int) []DirtyRect {
	type span struct {
		bx0, bx1, by int
	}
	var spans []span
	for by := range rows {
		bx := 0
		for bx < cols {
			if !dirty[by*cols+bx] {
				bx++
				continue
			}
			start := bx
			for bx < cols && dirty[by*cols+bx] {
				bx++
			}
			spans = append(spans, span{start, bx, by})
		}
	}

	type rect struct {
		bx0, bx1, by0, by1 int
	}
	var rects []rect
	used := make([]bool, len(spans))

	for i, s := range spans {
		if used[i] {
			continue
		}
		r := rect{s.bx0, s.bx1, s.by, s.by + 1}
		used[i] = true
		for j := i + 1; j < len(spans); j++ {
			if used[j] {
				continue
			}
			if spans[j].by == r.by1 && spans[j].bx0 == r.bx0 && spans[j].bx1 == r.bx1 {
				r.by1 = spans[j].by + 1
				used[j] = true
			}
		}
		rects = append(rects, r)
	}

	var result []DirtyRect
	for _, r := range rects {
		px := r.bx0 * bs
		py := r.by0 * bs
		pw := (r.bx1 - r.bx0) * bs
		ph := (r.by1 - r.by0) * bs
		if px+pw > imgW {
			pw = imgW - px
		}
		if py+ph > imgH {
			ph = imgH - py
		}
		result = append(result, DirtyRect{px, py, pw, ph})
	}
	return result
}

func (sd *ScreenDiffer) UpdateScreen(newImg *image.Gray, fullRefresh bool) error {
	sd.mu.Lock()
	defer sd.mu.Unlock()

	if fullRefresh {
		err := writeGray(newImg, true)
		sd.prevFrame = cloneGrayImage(newImg)
		return err
	}

	dirtyRects := sd.FindDirtyRects(sd.prevFrame, newImg)
	if len(dirtyRects) == 0 {
		log.Println("[diff] No changes detected, skipping screen update")
		return nil
	}

	totalPixels := newImg.Rect.Dx() * newImg.Rect.Dy()
	dirtyPixels := 0
	for _, r := range dirtyRects {
		dirtyPixels += r.W * r.H
	}
	ratio := float64(dirtyPixels) / float64(totalPixels)

	log.Printf("[diff] %d dirty regions, %.1f%% of screen changed", len(dirtyRects), ratio*100)

	if ratio > 0.60 || len(dirtyRects) > 5 {
		log.Printf("[diff] Too many changes (%.0f%%, %d rects), falling back to full update", ratio*100, len(dirtyRects))
		err := writeGray(newImg, false)
		sd.prevFrame = cloneGrayImage(newImg)
		return err
	}

	if err := writeGrayPartial(newImg, dirtyRects); err != nil {
		return err
	}

	sd.prevFrame = cloneGrayImage(newImg)
	return nil
}

func cloneGrayImage(src *image.Gray) *image.Gray {
	dst := image.NewGray(src.Rect)
	copy(dst.Pix, src.Pix)
	return dst
}

func (sd *ScreenDiffer) ClearDiffCache() {
	sd.mu.Lock()
	sd.prevFrame = nil
	sd.mu.Unlock()
}
