package main

import (
	"bytes"
	"fmt"
	"image"
	"image/png"
	"log"
	"os"
	"os/exec"
	"strconv"
	"sync"
	"time"
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

// FindDirtyRects 将新旧两帧按 blockSize 网格对比，返回所有脏块合并后的矩形列表
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

	for by := 0; by < rows; by++ {
		for bx := 0; bx < cols; bx++ {
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
		colEnd := rowStart + x0 + bs
		if colEnd > rowStart+imgW {
			colEnd = rowStart + imgW
		}
		if colStart >= len(oldImg.Pix) || colStart >= len(newImg.Pix) {
			continue
		}
		if colEnd > len(oldImg.Pix) {
			colEnd = len(oldImg.Pix)
		}
		if colEnd > len(newImg.Pix) {
			colEnd = len(newImg.Pix)
		}
		oldSlice := oldImg.Pix[colStart:colEnd]
		newSlice := newImg.Pix[colStart:colEnd]
		if !bytes.Equal(oldSlice, newSlice) {
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
	for by := 0; by < rows; by++ {
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

	eipsPath := "/usr/sbin/eips"
	hasEips := false
	if _, err := os.Stat(eipsPath); err == nil {
		hasEips = true
	}

	if fullRefresh {
		err := writeAndEips(newImg, eipsPath, hasEips, true)
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

	// >60% 或 >12 rects 时全屏更快（局部 eips 次数过多反而更慢）
	if ratio > 0.60 || len(dirtyRects) > 12 {
		log.Printf("[diff] Too many changes (%.0f%%, %d rects), falling back to full update", ratio*100, len(dirtyRects))
		err := writeAndEips(newImg, eipsPath, hasEips, false)
		sd.prevFrame = cloneGrayImage(newImg)
		return err
	}

	if hasEips {
		for i, r := range dirtyRects {
			if err := eipsPartialUpdate(newImg, eipsPath, r, i); err != nil {
				log.Printf("[diff] Partial update failed for rect %d, falling back: %v", i, err)
				_ = writeAndEips(newImg, eipsPath, hasEips, false)
				break
			}
		}
	} else {
		_ = writeAndEips(newImg, eipsPath, hasEips, false)
	}

	sd.prevFrame = cloneGrayImage(newImg)
	return nil
}

func writeAndEips(img *image.Gray, eipsPath string, hasEips, full bool) error {
	tmpPath := "/tmp/kkanpan.png"
	f, err := os.Create(tmpPath)
	if err != nil {
		return err
	}
	enc := &png.Encoder{CompressionLevel: png.BestSpeed}
	if err := enc.Encode(f, img); err != nil {
		f.Close()
		return err
	}
	f.Close()

	if hasEips {
		if full {
			_ = exec.Command(eipsPath, "-c").Run()
			time.Sleep(200 * time.Millisecond)
			cmd := exec.Command(eipsPath, "-f", "-g", tmpPath)
			if out, err := cmd.CombinedOutput(); err != nil {
				log.Printf("eips -f -g err: %v, output: %s, retrying -g", err, string(out))
				_ = exec.Command(eipsPath, "-g", tmpPath).Run()
			}
		} else {
			cmd := exec.Command(eipsPath, "-g", tmpPath)
			if out, err := cmd.CombinedOutput(); err != nil {
				log.Printf("eips -g err: %v, output: %s", err, string(out))
			}
		}
		log.Println("Kindle eips refreshed successfully")
	} else {
		log.Printf("Screen image rendered to %s (not on Kindle device)", tmpPath)
	}
	return nil
}

func eipsPartialUpdate(img *image.Gray, eipsPath string, r DirtyRect, idx int) error {
	cropped := image.NewGray(image.Rect(0, 0, r.W, r.H))
	for y := 0; y < r.H; y++ {
		srcOff := (r.Y+y)*img.Stride + r.X
		dstOff := y * cropped.Stride
		copy(cropped.Pix[dstOff:dstOff+r.W], img.Pix[srcOff:srcOff+r.W])
	}

	tmpPath := fmt.Sprintf("/tmp/kkanpan_patch_%d.png", idx)
	f, err := os.Create(tmpPath)
	if err != nil {
		return err
	}
	enc := &png.Encoder{CompressionLevel: png.BestSpeed}
	if err := enc.Encode(f, cropped); err != nil {
		f.Close()
		return err
	}
	f.Close()

	cmd := exec.Command(eipsPath, "-g", tmpPath,
		"-x", strconv.Itoa(r.X),
		"-y", strconv.Itoa(r.Y))
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("eips partial: %v (output: %s)", err, string(out))
	}
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
