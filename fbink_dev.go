//go:build !kindle

package main

import (
	"image"
	"image/png"
	"log"
	"os"
	"os/exec"
	"strconv"
)

func initDisplay() error {
	log.Println("[display] Dev mode: PNG+eips fallback")
	return nil
}

func writeGray(img *image.Gray, full bool) error {
	tmpPath := "/tmp/kkanpan.png"
	f, err := os.Create(tmpPath)
	if err != nil {
		return err
	}
	enc := &png.Encoder{CompressionLevel: png.NoCompression}
	if err := enc.Encode(f, img); err != nil {
		f.Close()
		return err
	}
	f.Close()

	eipsPath := "/usr/sbin/eips"
	if _, err := os.Stat(eipsPath); err != nil {
		log.Printf("[display] Rendered to %s (no eips)", tmpPath)
		return nil
	}

	if full {
		_ = exec.Command(eipsPath, "-c").Run()
		cmd := exec.Command(eipsPath, "-f", "-g", tmpPath)
		if out, err := cmd.CombinedOutput(); err != nil {
			log.Printf("[display] eips -f -g err: %v, output: %s", err, string(out))
			_ = exec.Command(eipsPath, "-g", tmpPath).Run()
		}
	} else {
		cmd := exec.Command(eipsPath, "-g", tmpPath)
		if out, err := cmd.CombinedOutput(); err != nil {
			log.Printf("[display] eips -g err: %v, output: %s", err, string(out))
		}
	}
	return nil
}

func writeGrayPartial(img *image.Gray, rects []DirtyRect) error {
	eipsPath := "/usr/sbin/eips"
	if _, err := os.Stat(eipsPath); err != nil {
		return writeGray(img, false)
	}

	for i, r := range rects {
		cropped := image.NewGray(image.Rect(0, 0, r.W, r.H))
		for y := 0; y < r.H; y++ {
			srcOff := (r.Y+y)*img.Stride + r.X
			dstOff := y * cropped.Stride
			copy(cropped.Pix[dstOff:dstOff+r.W], img.Pix[srcOff:srcOff+r.W])
		}

		tmpPath := "/tmp/kkanpan_patch_" + strconv.Itoa(i) + ".png"
		f, err := os.Create(tmpPath)
		if err != nil {
			continue
		}
		png.Encode(f, cropped)
		f.Close()

		cmd := exec.Command(eipsPath, "-g", tmpPath,
			"-x", strconv.Itoa(r.X),
			"-y", strconv.Itoa(r.Y))
		if out, err := cmd.CombinedOutput(); err != nil {
			log.Printf("[display] eips partial err: %v, output: %s", err, string(out))
			return writeGray(img, false)
		}
	}
	return nil
}

func clearScreen() error {
	eipsPath := "/usr/sbin/eips"
	if _, err := os.Stat(eipsPath); err == nil {
		return exec.Command(eipsPath, "-c").Run()
	}
	return nil
}

func closeDisplay() {}

func dumpFB() {
	_ = exec.Command("sh", "-c", "cat /dev/fb0 > /var/tmp/kkanpan-fb.dump 2>/dev/null").Run()
}

func restoreFB() {
	_ = exec.Command("sh", "-c", "cat /var/tmp/kkanpan-fb.dump > /dev/fb0 2>/dev/null; rm -f /var/tmp/kkanpan-fb.dump").Run()
}
