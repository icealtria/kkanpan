//go:build kindle

package main

import (
	"image"
	"log"

	"github.com/shermp/go-fbink-v2/v2/gofbink"
)

var fb *gofbink.FBInk

func initDisplay() error {
	cfg := gofbink.FBInkConfig{
		IsFlashing: false,
		WfmMode:    gofbink.WfmAUTO,
	}
	rCfg := gofbink.RestrictedConfig{
		Fontmult: 1,
		Fontname: gofbink.IBM,
	}

	fb = gofbink.New(&cfg, &rCfg)
	fb.Open()
	fb.Init(&cfg)

	log.Printf("[display] FBInk %s initialized", fb.Version())
	return nil
}

func writeGray(img *image.Gray, full bool) error {
	cfg := gofbink.FBInkConfig{
		WfmMode: gofbink.WfmDU,
	}
	if full {
		cfg.IsFlashing = true
		cfg.WfmMode = gofbink.WfmGC16
	}

	w := img.Rect.Dx()
	h := img.Rect.Dy()
	return fb.PrintRawData(img.Pix, w, h, 0, 0, &cfg)
}

func writeGrayPartial(img *image.Gray, rects []DirtyRect) error {
	for _, r := range rects {
		cropped := make([]byte, r.W*r.H)
		for y := 0; y < r.H; y++ {
			srcOff := (r.Y+y)*img.Stride + r.X
			copy(cropped[y*r.W:(y+1)*r.W], img.Pix[srcOff:srcOff+r.W])
		}

		cfg := gofbink.FBInkConfig{
			WfmMode: gofbink.WfmDU,
		}
		if err := fb.PrintRawData(cropped, r.W, r.H, uint16(r.X), uint16(r.Y), &cfg); err != nil {
			log.Printf("[display] partial update failed at (%d,%d): %v", r.X, r.Y, err)
			return writeGray(img, false)
		}
	}
	return nil
}

func clearScreen() error {
	cfg := gofbink.FBInkConfig{}
	return fb.ClearScreen(&cfg, nil)
}

func closeDisplay() {
	if fb != nil {
		fb.Close()
	}
}

func dumpFB() {
	log.Println("[display] dumpFB: FBInk does not support dump, skipping")
}

func restoreFB() {
	log.Println("[display] restoreFB: FBInk does not support restore, skipping")
}
