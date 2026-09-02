package main

import (
	"bytes"
	"encoding/binary"
	"io"
	"log"
	"math"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"
)

type inputEvent32 struct {
	Sec   int32
	Usec  int32
	Type  uint16
	Code  uint16
	Value int32
}

const (
	EV_SYN = 0x00
	EV_KEY = 0x01
	EV_ABS = 0x03

	ABS_X              = 0x00
	ABS_Y              = 0x01
	ABS_MT_POSITION_X  = 0x35
	ABS_MT_POSITION_Y  = 0x36
	ABS_MT_TRACKING_ID = 0x39
	BTN_TOUCH          = 0x14a

	// Linux ioctl: _IOW('E', 0x90, int) -> 独占触屏设备，拦截背景系统的一切点击
	EVIOCGRAB = 0x40044590
)

var (
	currentViewMode  = "AUTO" // AUTO, ALL or any group from stocks.json
	viewModeMu       sync.RWMutex
	triggerRefreshCh = make(chan bool, 1)
	grabbedDevFile   *os.File
)

func GetViewMode() string {
	viewModeMu.RLock()
	defer viewModeMu.RUnlock()
	return currentViewMode
}

func SetViewMode(mode string) {
	viewModeMu.Lock()
	currentViewMode = mode
	viewModeMu.Unlock()
	ResetPage()
	screenDiffer.ClearDiffCache()
	select {
	case triggerRefreshCh <- true:
	default:
	}
}

func triggerPageRefresh() {
	screenDiffer.ClearDiffCache()
	select {
	case triggerRefreshCh <- true:
	default:
	}
}

func quitApp() {
	log.Println("Exit requested. Restoring Kindle state and exiting...")
	if grabbedDevFile != nil {
		_, _, _ = syscall.Syscall(syscall.SYS_IOCTL, grabbedDevFile.Fd(), uintptr(EVIOCGRAB), 0)
		grabbedDevFile.Close()
	}
	EnableCoexistMode()
	// 唤醒 Kindle 原生主页并清屏重绘
	_ = exec.Command("lipc-set-prop", "com.lab126.appmgrd", "show", "app://com.lab126.booklet.home").Run()
	eipsPath := "/usr/sbin/eips"
	if _, err := os.Stat(eipsPath); err == nil {
		_ = exec.Command(eipsPath, "-c").Run()
	}
	time.Sleep(300 * time.Millisecond)
	os.Exit(0)
}

func parseHM(s string) int {
	parts := strings.Split(s, ":")
	if len(parts) != 2 {
		return 0
	}
	h, _ := strconv.Atoi(parts[0])
	m, _ := strconv.Atoi(parts[1])
	return h*60 + m
}

func matchRule(now time.Time, r AutoRule) bool {
	if len(r.Weekdays) > 0 {
		wd := int(now.Weekday())
		matched := false
		for _, w := range r.Weekdays {
			if w == wd {
				matched = true
				break
			}
		}
		if !matched {
			return false
		}
	}
	start := parseHM(r.Start)
	end := parseHM(r.End)
	cur := now.Hour()*60 + now.Minute()
	if start <= end {
		return cur >= start && cur <= end
	}
	return cur >= start || cur <= end
}

func GetEffectiveGroup(mode string) (groupName string, isAuto bool) {
	if mode != "AUTO" && mode != "" {
		return mode, false
	}
	if len(appConfig.AutoRules) == 0 {
		return "", true
	}
	loc := time.FixedZone("CST", 8*3600)
	now := time.Now().In(loc)
	for _, r := range appConfig.AutoRules {
		if matchRule(now, r) {
			return r.Group, true
		}
	}
	return "", true
}

func startTouchListener(screenWidth, screenHeight int) {
	var devPath string
	for _, p := range []string{"/dev/input/event1", "/dev/input/event0", "/dev/input/event2"} {
		if _, err := os.Stat(p); err == nil {
			devPath = p
			break
		}
	}
	if devPath == "" {
		log.Println("Touch device not found, skipping touch listener.")
		return
	}

	file, err := os.OpenFile(devPath, os.O_RDWR, 0)
	if err != nil {
		file, err = os.Open(devPath)
	}
	if err != nil {
		log.Printf("Cannot open touch device %s: %v", devPath, err)
		return
	}
	defer file.Close()
	grabbedDevFile = file

	// 关键：对触控设备调用 EVIOCGRAB 独占！阻止背景系统 (X11 / cvm / KUAL) 接收任何点击事件！
	_, _, errno := syscall.Syscall(syscall.SYS_IOCTL, file.Fd(), uintptr(EVIOCGRAB), 1)
	if errno != 0 {
		log.Printf("EVIOCGRAB exclusive grab status: %v (might be non-fatal)", errno)
	} else {
		log.Println("Successfully acquired EXCLUSIVE grab on touchscreen! Background input completely blocked.")
	}

	log.Printf("Touch listener active on %s (Resolution: %dx%d)", devPath, screenWidth, screenHeight)

	buf := make([]byte, 16)
	var curX, curY, startX, startY int
	var startTime time.Time
	touching := false

	handleTap := func(x, y int) {
		log.Printf("Touch tap detected at (%d, %d)", x, y)

		// 1. 右上角 [X] 关闭按钮判定 (区域加大，更好点击)
		if x >= screenWidth-120 && y <= 75 {
			log.Println("Close [X] button tapped!")
			quitApp()
			return
		}

		// 2. 顶部 Tab 栏判定 (y: 60 ~ 135) — 动态 Tab
		if y >= 60 && y <= 135 {
			modes := GetTabModes()
			tabTotalW := screenWidth - 60
			tabCount := len(modes)
			tabW := tabTotalW / tabCount
			idx := (x - 30) / tabW
			if idx >= 0 && idx < len(modes) {
				log.Printf("Switched view mode to: %s", modes[idx])
				SetViewMode(modes[idx])
			}
			return
		}

		// 3. 底部点击触发立即刷新 (y > screenHeight - 100) — 保留但与翻页手势区分
		if y >= screenHeight-100 {
			// 底部左右分區: 左1/3 上一页, 右1/3 下一页, 中间刷新
			if x < screenWidth/3 {
				if PrevPage() {
					log.Printf("Prev page -> %d", GetCurrentPage()+1)
					triggerPageRefresh()
				}
				return
			}
			if x > screenWidth*2/3 {
				var total int
				cacheMutex.RLock()
				if len(cachedData) > 0 {
					total = GetTotalPages(cachedData, screenWidth, screenHeight)
				} else {
					total = GetTotalPages(neededStocksData(), screenWidth, screenHeight)
				}
				cacheMutex.RUnlock()
				if total == 0 {
					total = 1
				}
				if NextPage(total) {
					log.Printf("Next page -> %d/%d", GetCurrentPage()+1, total)
					triggerPageRefresh()
				} else {
					log.Println("Bottom touched: triggering refresh")
					select {
					case triggerRefreshCh <- true:
					default:
					}
				}
				return
			}
			log.Println("Bottom touched: Triggering immediate refresh...")
			select {
			case triggerRefreshCh <- true:
			default:
			}
			return
		}
	}

	handleSwipe := func(sx, sy, ex, ey int, dur time.Duration) bool {
		dx := ex - sx
		dy := ey - sy
		adx := dx
		if adx < 0 {
			adx = -adx
		}
		ady := dy
		if ady < 0 {
			ady = -ady
		}
		if dur > 700*time.Millisecond || dur < 80*time.Millisecond {
			return false
		}
		// 轻触容差: KOReader PAN_THRESHOLD≈63px@300ppi, 漂移<12px视为tap不判swipe
		if adx < 12 && ady < 12 {
			return false
		}
		// 速度 = 欧氏距离 / 时长, 需 >0.4px/ms (400px/s) 才算有意滑动, 过滤慢拖误触
		dist := math.Sqrt(float64(dx*dx + dy*dy))
		ms := float64(dur.Milliseconds())
		if ms < 1 {
			ms = 1
		}
		velocity := dist / ms // px/ms
		if velocity < 0.4 {
			return false
		}
		// 仅保留垂直滑动翻页，水平手势已移除（易误触，Tab 改为点按切换）
		if ady > 120 && ady > adx*2 {
			var total int
			cacheMutex.RLock()
			if len(cachedData) > 0 {
				total = GetTotalPages(cachedData, screenWidth, screenHeight)
			} else {
				total = GetTotalPages(neededStocksData(), screenWidth, screenHeight)
			}
			cacheMutex.RUnlock()
			if total <= 1 {
				return false
			}
			if dy < 0 {
				if NextPage(total) {
					log.Printf("Swipe up: next page %d/%d", GetCurrentPage()+1, total)
					triggerPageRefresh()
					return true
				}
			} else {
				if PrevPage() {
					log.Printf("Swipe down: prev page %d/%d", GetCurrentPage()+1, total)
					triggerPageRefresh()
					return true
				}
			}
			return false
		}
		return false
	}

	for {
		_, err := io.ReadFull(file, buf)
		if err != nil {
			time.Sleep(1 * time.Second)
			continue
		}

		var evType, evCode uint16
		var evVal int32

		var ev32 inputEvent32
		if err := binary.Read(bytes.NewReader(buf), binary.LittleEndian, &ev32); err == nil {
			evType = ev32.Type
			evCode = ev32.Code
			evVal = ev32.Value
		}

		if evType == EV_ABS {
			if evCode == ABS_MT_POSITION_X || evCode == ABS_X {
				curX = int(evVal)
			} else if evCode == ABS_MT_POSITION_Y || evCode == ABS_Y {
				curY = int(evVal)
			}
		} else if evType == EV_KEY && evCode == BTN_TOUCH {
			if evVal == 1 {
				touching = true
				startX, startY = curX, curY
				startTime = time.Now()
			} else if evVal == 0 && touching {
				touching = false
				if curX == 0 && curY == 0 {
					curX, curY = startX, startY
				}
				if !startTime.IsZero() {
					if handleSwipe(startX, startY, curX, curY, time.Since(startTime)) {
						startX, startY = 0, 0
						startTime = time.Time{}
						continue
					}
				}
				if curX > 0 && curY > 0 {
					go handleTap(curX, curY)
				}
				startX, startY = 0, 0
				startTime = time.Time{}
			}
		}
	}
}
