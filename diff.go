package main

import (
	"bytes"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"log"
	"os"
	"os/exec"
	"sync"
	"time"
)

// DirtyRect 表示一个需要更新的脏矩形区域
type DirtyRect struct {
	X, Y, W, H int
}

// ScreenDiffer 负责帧间 diff，只更新变化的区域
type ScreenDiffer struct {
	mu        sync.Mutex
	prevFrame *image.Gray
	blockSize int // 每个检测块的大小 (像素)
}

var screenDiffer = &ScreenDiffer{
	blockSize: 8, // 8x8 像素块检测粒度，平衡精度与性能
}

// FindDirtyRects 将新旧两帧按 blockSize 网格对比，返回所有脏块合并后的矩形列表
func (sd *ScreenDiffer) FindDirtyRects(oldImg, newImg *image.Gray) []DirtyRect {
	if oldImg == nil {
		// 首帧，整屏都是脏的
		return []DirtyRect{{0, 0, newImg.Rect.Dx(), newImg.Rect.Dy()}}
	}

	w, h := newImg.Rect.Dx(), newImg.Rect.Dy()
	ow, oh := oldImg.Rect.Dx(), oldImg.Rect.Dy()
	if w != ow || h != oh {
		// 分辨率变了，整屏刷新
		return []DirtyRect{{0, 0, w, h}}
	}

	bs := sd.blockSize
	cols := (w + bs - 1) / bs
	rows := (h + bs - 1) / bs

	// 标记脏块
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
		return nil // 完全没变化
	}

	// 合并相邻脏块为较大的矩形 (贪心行扫描)
	return mergeBlocks(dirty, cols, rows, bs, w, h)
}

// isBlockDirty 逐字节比较一个块的像素是否有变化
func (sd *ScreenDiffer) isBlockDirty(oldImg, newImg *image.Gray, x0, y0, bs, imgW, imgH int) bool {
	// 直接比较 Pix slice 中对应的字节段（性能关键路径）
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

// mergeBlocks 把脏块网格合并成尽量少的矩形
// 策略：行方向上先合并连续脏块为行条，再把垂直相邻且 x 范围完全相同的行条纵向合并
func mergeBlocks(dirty []bool, cols, rows, bs, imgW, imgH int) []DirtyRect {
	// 第一步：提取行条 (row spans)
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

	// 第二步：纵向合并相邻行条
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
		// 向下查找可合并的行条
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

	// 转换为像素坐标
	var result []DirtyRect
	for _, r := range rects {
		px := r.bx0 * bs
		py := r.by0 * bs
		pw := (r.bx1 - r.bx0) * bs
		ph := (r.by1 - r.by0) * bs
		// 裁剪到屏幕边界
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

// UpdateScreen 用 diff 策略刷新 Kindle 屏幕
// 如果 fullRefresh=true，则忽略 diff 做全屏刷新
func (sd *ScreenDiffer) UpdateScreen(newImg *image.Gray, fullRefresh bool) error {
	sd.mu.Lock()
	defer sd.mu.Unlock()

	eipsPath := "/usr/sbin/eips"
	hasEips := false
	if _, err := os.Stat(eipsPath); err == nil {
		hasEips = true
	}

	if fullRefresh {
		// 全屏刷新: 直接调 eips -f -g
		err := writeAndEips(newImg, eipsPath, hasEips, true)
		sd.prevFrame = cloneGrayImage(newImg)
		return err
	}

	// diff 模式
	dirtyRects := sd.FindDirtyRects(sd.prevFrame, newImg)
	if len(dirtyRects) == 0 {
		log.Println("[diff] No changes detected, skipping screen update")
		return nil
	}

	// 计算脏面积占比
	totalPixels := newImg.Rect.Dx() * newImg.Rect.Dy()
	dirtyPixels := 0
	for _, r := range dirtyRects {
		dirtyPixels += r.W * r.H
	}
	ratio := float64(dirtyPixels) / float64(totalPixels)

	log.Printf("[diff] %d dirty regions, %.1f%% of screen changed", len(dirtyRects), ratio*100)

	// 如果脏区域超过 60% 或区域数过多，退化为全屏更新（局部 eips 调用次数太多反而更慢）
	if ratio > 0.60 || len(dirtyRects) > 12 {
		log.Printf("[diff] Too many changes (%.0f%%, %d rects), falling back to full update", ratio*100, len(dirtyRects))
		err := writeAndEips(newImg, eipsPath, hasEips, false)
		sd.prevFrame = cloneGrayImage(newImg)
		return err
	}

	// 局部更新
	if hasEips {
		for i, r := range dirtyRects {
			if err := eipsPartialUpdate(newImg, eipsPath, r, i); err != nil {
				log.Printf("[diff] Partial update failed for rect %d, falling back: %v", i, err)
				_ = writeAndEips(newImg, eipsPath, hasEips, false)
				break
			}
		}
	} else {
		// 不在 Kindle 上，写全图供调试
		_ = writeAndEips(newImg, eipsPath, hasEips, false)
	}

	sd.prevFrame = cloneGrayImage(newImg)
	return nil
}

// writeAndEips 写入完整 PNG 并调用 eips
func writeAndEips(img *image.Gray, eipsPath string, hasEips, full bool) error {
	tmpPath := "/tmp/kkanpan.png"
	f, err := os.Create(tmpPath)
	if err != nil {
		return err
	}
	if err := png.Encode(f, img); err != nil {
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

// eipsPartialUpdate 裁剪脏区域为子图 PNG，用 eips 局部刷新
func eipsPartialUpdate(img *image.Gray, eipsPath string, r DirtyRect, idx int) error {
	// 裁剪子图
	subImg := img.SubImage(image.Rect(r.X, r.Y, r.X+r.W, r.Y+r.H)).(*image.Gray)

	// 需要创建一个新的 image.Gray，origin 归零，否则 PNG 坐标不对
	cropped := image.NewGray(image.Rect(0, 0, r.W, r.H))
	for y := 0; y < r.H; y++ {
		srcOff := (r.Y+y)*img.Stride + r.X
		dstOff := y * cropped.Stride
		copy(cropped.Pix[dstOff:dstOff+r.W], img.Pix[srcOff:srcOff+r.W])
	}
	_ = subImg // just for reference

	tmpPath := fmt.Sprintf("/tmp/kkanpan_patch_%d.png", idx)
	f, err := os.Create(tmpPath)
	if err != nil {
		return err
	}
	if err := png.Encode(f, cropped); err != nil {
		f.Close()
		return err
	}
	f.Close()

	// eips -g <file> -x <col> -y <row>
	// eips 的 -x/-y 参数单位是像素坐标
	cmd := exec.Command(eipsPath, "-g", tmpPath,
		fmt.Sprintf("-x"), fmt.Sprintf("%d", r.X),
		fmt.Sprintf("-y"), fmt.Sprintf("%d", r.Y))
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("eips partial: %v (output: %s)", err, string(out))
	}
	return nil
}

// cloneGrayImage 深拷贝一个 Gray 图像
func cloneGrayImage(src *image.Gray) *image.Gray {
	dst := image.NewGray(src.Rect)
	copy(dst.Pix, src.Pix)
	return dst
}

// ClearDiffCache 清除上一帧缓存，强制下次全屏刷新
func (sd *ScreenDiffer) ClearDiffCache() {
	sd.mu.Lock()
	sd.prevFrame = nil
	sd.mu.Unlock()
}

// ============================================================
// 按行快速 diff 统计（用于日志/调试）
// ============================================================

// DiffStats 返回新旧帧的差异统计
type DiffStats struct {
	TotalPixels   int
	ChangedPixels int
	DirtyRects    int
	ChangeRatio   float64
}

func (sd *ScreenDiffer) GetDiffStats(oldImg, newImg *image.Gray) DiffStats {
	stats := DiffStats{
		TotalPixels: newImg.Rect.Dx() * newImg.Rect.Dy(),
	}
	if oldImg == nil {
		stats.ChangedPixels = stats.TotalPixels
		stats.DirtyRects = 1
		stats.ChangeRatio = 1.0
		return stats
	}

	w, h := newImg.Rect.Dx(), newImg.Rect.Dy()
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			if oldImg.GrayAt(x, y) != newImg.GrayAt(x, y) {
				stats.ChangedPixels++
			}
		}
	}

	rects := sd.FindDirtyRects(oldImg, newImg)
	stats.DirtyRects = len(rects)
	if stats.TotalPixels > 0 {
		stats.ChangeRatio = float64(stats.ChangedPixels) / float64(stats.TotalPixels)
	}
	return stats
}

// fillRectOnImage 是一个便捷方法用于在 image 上画填充矩形（用于调试可视化脏区域边框）
func drawDirtyOverlay(img *image.Gray, rects []DirtyRect) {
	for _, r := range rects {
		// 画红色（黑色在灰度下）边框来标识脏区域
		for x := r.X; x < r.X+r.W && x < img.Rect.Dx(); x++ {
			if r.Y >= 0 && r.Y < img.Rect.Dy() {
				img.SetGray(x, r.Y, color.Gray{Y: 80})
			}
			bottom := r.Y + r.H - 1
			if bottom >= 0 && bottom < img.Rect.Dy() {
				img.SetGray(x, bottom, color.Gray{Y: 80})
			}
		}
		for y := r.Y; y < r.Y+r.H && y < img.Rect.Dy(); y++ {
			if r.X >= 0 && r.X < img.Rect.Dx() {
				img.SetGray(r.X, y, color.Gray{Y: 80})
			}
			right := r.X + r.W - 1
			if right >= 0 && right < img.Rect.Dx() {
				img.SetGray(right, y, color.Gray{Y: 80})
			}
		}
	}
}
