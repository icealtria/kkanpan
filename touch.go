package main

import (
	"bytes"
	"encoding/binary"
	"io"
	"log"
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
	currentViewMode  = "AUTO" // AUTO, A股, 美股, 期货, 全部
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
	select {
	case triggerRefreshCh <- true:
	default:
	}
}

// 退出应用并恢复 Kindle 原生桌面 (共存模式, 不重启)
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
		return "全部", true
	}
	loc := time.FixedZone("CST", 8*3600)
	now := time.Now().In(loc)
	for _, r := range appConfig.AutoRules {
		if matchRule(now, r) {
			return r.Group, true
		}
	}
	return "全部", true
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
	var curX, curY int
	touching := false

	handleTap := func(x, y int) {
		log.Printf("Touch tap detected at (%d, %d)", x, y)

		// 1. 右上角 [X] 关闭按钮判定 (区域加大，更好点击)
		if x >= screenWidth-120 && y <= 75 {
			log.Println("Close [X] button tapped!")
			quitApp()
			return
		}

		// 2. 顶部 Tab 栏判定 (y: 60 ~ 135)
		if y >= 60 && y <= 135 {
			tabTotalW := screenWidth - 60
			tabCount := 5
			tabW := tabTotalW / tabCount
			idx := (x - 30) / tabW
			modes := []string{"AUTO", "A股", "美股", "期货", "全部"}
			if idx >= 0 && idx < len(modes) {
				log.Printf("Switched view mode to: %s", modes[idx])
				SetViewMode(modes[idx])
			}
			return
		}

		// 3. 底部点击触发立即刷新 (y > screenHeight - 100)
		if y >= screenHeight-100 {
			log.Println("Bottom touched: Triggering immediate refresh...")
			select {
			case triggerRefreshCh <- true:
			default:
			}
			return
		}
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
			} else if evVal == 0 && touching {
				touching = false
				if curX > 0 && curY > 0 {
					go handleTap(curX, curY)
				}
			}
		}
	}
}
