package main

import (
	"log"
	"os"
	"os/exec"
	"strings"
	"sync"
	"time"
)

// KindleSystemInfo 保存从系统读取的设备状态
type KindleSystemInfo struct {
	BatteryLevel string // e.g. "85"
	IsCharging   bool
	WiFiSignal   string // e.g. "3/4" 或 "OFF"
	Time         string // HH:MM
}

var (
	sysInfoCache   KindleSystemInfo
	sysInfoMu      sync.RWMutex
	sysInfoUpdated int64
)

// GetKindleSystemInfo 返回缓存的系统信息，最多每 30 秒刷新一次
func GetKindleSystemInfo() KindleSystemInfo {
	sysInfoMu.RLock()
	now := time.Now().Unix()
	if now-sysInfoUpdated < 30 && sysInfoUpdated > 0 {
		defer sysInfoMu.RUnlock()
		return sysInfoCache
	}
	sysInfoMu.RUnlock()

	info := readKindleSystemInfo()
	sysInfoMu.Lock()
	sysInfoCache = info
	sysInfoUpdated = now
	sysInfoMu.Unlock()
	return info
}

func readKindleSystemInfo() KindleSystemInfo {
	info := KindleSystemInfo{
		Time: time.Now().Format("15:04"),
	}

	// 读取电池电量
	info.BatteryLevel = lipcGet("com.lab126.powerd", "battLevel")
	if info.BatteryLevel == "" {
		info.BatteryLevel = "--"
	}

	// 读取充电状态
	charging := lipcGet("com.lab126.powerd", "isCharging")
	info.IsCharging = (charging == "1")

	// 读取 WiFi 信号强度
	signal := lipcGet("com.lab126.wifid", "signalStrength")
	if signal == "" {
		// 尝试读取连接状态判断 WiFi 是否开启
		cmState := lipcGet("com.lab126.wifid", "cmState")
		if cmState == "" || cmState == "DOWN" || cmState == "NA" {
			info.WiFiSignal = "OFF"
		} else {
			info.WiFiSignal = "?"
		}
	} else {
		info.WiFiSignal = signalToBar(signal)
	}

	return info
}

// lipcGet 执行 lipc-get-prop 命令并返回 trim 后的结果
func lipcGet(service, prop string) string {
	out, err := exec.Command("lipc-get-prop", service, prop).Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

// signalToBar 将信号强度数值转为可读的信号条表示
func signalToBar(raw string) string {
	// Kindle signalStrength 通常返回 0-100 的 RSSI 百分比值
	// 也可能返回负的 dBm 值如 -67
	raw = strings.TrimSpace(raw)
	if raw == "" || raw == "0" {
		return "OFF"
	}

	// 尝试解析
	var level int
	n, _ := parseSignalInt(raw)
	if n < 0 {
		// dBm 值: -30 极好, -67 好, -70 中等, -80 弱, -90 极弱
		if n >= -50 {
			level = 4
		} else if n >= -65 {
			level = 3
		} else if n >= -75 {
			level = 2
		} else {
			level = 1
		}
	} else {
		// 百分比 0-100
		if n >= 75 {
			level = 4
		} else if n >= 50 {
			level = 3
		} else if n >= 25 {
			level = 2
		} else {
			level = 1
		}
	}

	bars := []string{"▁", "▁▃", "▁▃▅", "▁▃▅▇"}
	if level >= 1 && level <= 4 {
		return bars[level-1]
	}
	return "?"
}

func parseSignalInt(s string) (int, bool) {
	var v int
	negative := false
	if len(s) > 0 && s[0] == '-' {
		negative = true
		s = s[1:]
	}
	if len(s) == 0 {
		return 0, false
	}
	for _, ch := range s {
		if ch < '0' || ch > '9' {
			return 0, false
		}
		v = v*10 + int(ch-'0')
	}
	if negative {
		v = -v
	}
	return v, true
}

// FormatStatusBar 格式化底部状态栏文本
func FormatStatusBar() string {
	info := GetKindleSystemInfo()

	battStr := info.BatteryLevel + "%"
	if info.IsCharging {
		battStr = info.BatteryLevel + "% CHG"
	}

	return info.Time + " | WIFI " + info.WiFiSignal + " | BATT " + battStr
}

// ============================================================
// Kindle UI 控制: 共存模式 (不杀 framework)
// 逻辑对齐 KOReader platform/kindle/koreader.sh, 退出不重启、无桌面日志残留
// ============================================================

var (
	kindlePillowHardDisabled bool
	kindleAwesomeStopped   bool
	kindleCvmStopped       bool
	kindleVolumdStopped    bool
	kindleStatusbarStopped bool
	kindleWmctrlUsed       bool
	kindleTitlebarGeom     string
	kindleInitType         string // "upstart" | "sysv"
	kindleFWVersion        string
)

func init() {
	if _, err := os.Stat("/etc/upstart"); err == nil {
		kindleInitType = "upstart"
	} else {
		kindleInitType = "sysv"
	}
	kindleFWVersion = readFWVersion()
}

func readFWVersion() string {
	out, err := exec.Command("sh", "-c", "grep '^Kindle 5' /etc/prettyversion.txt 2>/dev/null | sed -n -r 's/^(Kindle)([[:blank:]]*)([[:digit:]\\.]*)(.*?)$/\\3/p'").Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func versionGE(a, b string) bool {
	// a >= b ? 复刻 koreader.sh version() awk 逻辑: %d%03d%03d
	parse := func(s string) int {
		parts := strings.Split(s, ".")
		v := 0
		for i := 0; i < 3; i++ {
			n := 0
			if i < len(parts) {
				for _, ch := range parts[i] {
					if ch >= '0' && ch <= '9' {
						n = n*10 + int(ch-'0')
					} else {
						break
					}
				}
			}
			if i == 0 {
				v += n * 1000000
			} else if i == 1 {
				v += n * 1000
			} else {
				v += n
			}
		}
		return v
	}
	return parse(a) >= parse(b)
}

func dumpFB() {
	_ = exec.Command("sh", "-c", "cat /dev/fb0 > /var/tmp/kkanpan-fb.dump 2>/dev/null").Run()
}

func restoreFB() {
	_ = exec.Command("sh", "-c", "cat /var/tmp/kkanpan-fb.dump > /dev/fb0 2>/dev/null; rm -f /var/tmp/kkanpan-fb.dump").Run()
}

// DisablePillow 共存模式入口 (兼容旧调用) -> 转调共存模式
func DisablePillow() {
	DisableCoexistMode()
}

// EnablePillow 共存模式恢复 (兼容旧调用)
func EnablePillow() {
	EnableCoexistMode()
}

// DisableCoexistMode 共存模式: 不杀 framework, 通过 pillow+awesome+wmctrl+statusbar 屏蔽状态栏
func DisableCoexistMode() {
	_ = exec.Command("lipc-set-prop", "-i", "com.lab126.powerd", "preventScreenSaver", "1").Run()
	log.Println("[UI] preventScreenSaver=1")

	if kindleInitType == "upstart" {
		dumpFB()

		if kindleFWVersion != "" && versionGE(kindleFWVersion, "5.6.5") {
			log.Printf("[UI] FW %s >=5.6.5 hard-disabling pillow", kindleFWVersion)
			_ = exec.Command("lipc-set-prop", "com.lab126.pillow", "disableEnablePillow", "disable").Run()
			kindlePillowHardDisabled = true

			if versionGE(kindleFWVersion, "5.7.2") {
				// wmctrl resize (仅 <5.12.4, 否则有 softlock 风险 #6117)
				if versionGE(kindleFWVersion, "5.12.4") {
					log.Println("[UI] FW >=5.12.4 skip wmctrl resize (KOReader #6117)")
				} else {
					kindleTitlebarGeom = getTitlebarGeometry()
					if kindleTitlebarGeom != "" {
						if err := exec.Command("sh", "-c", wmctrlCmd("-r", ":titleBar_ID:", "-e", kindleTitlebarGeom+",1")).Run(); err == nil {
							kindleWmctrlUsed = true
							log.Printf("[UI] wmctrl resized titleBar %s -> 1px", kindleTitlebarGeom)
						}
					}
				}
				// SIGSTOP awesome (KPW3 的 WM), 比 pillow 更关键: 阻止每分钟 flashTimeoutExpired 重绘
				if exec.Command("killall", "-STOP", "awesome").Run() == nil {
					kindleAwesomeStopped = true
					log.Println("[UI] awesome STOPPED (status bar frozen)")
				} else {
					// 兜底: 某些固件 WM 叫 cvm
					if exec.Command("killall", "-STOP", "cvm").Run() == nil {
						kindleCvmStopped = true
						log.Println("[UI] cvm STOPPED (fallback)")
					}
				}
				time.Sleep(250 * time.Millisecond)
			}
		} else {
			// 老固件软隐藏
			log.Println("[UI] FW <5.6.5 soft-hiding status bar")
			_ = exec.Command("lipc-set-prop", "com.lab126.pillow", "interrogatePillow", `{"pillowId": "default_status_bar", "function": "nativeBridge.hideMe();"}`).Run()
		}

		// 额外: 新固件独立 statusbar job (PW6 @5.17+, KPW3 部分 5.16+)
		if _, err := os.Stat("/etc/upstart/statusbar.conf"); err == nil {
			if exec.Command("stop", "statusbar").Run() == nil {
				kindleStatusbarStopped = true
				log.Println("[UI] statusbar job stopped")
			}
		}
	}

	// sysv 额外: STOP cvm
	if kindleInitType == "sysv" {
		if exec.Command("killall", "-STOP", "cvm").Run() == nil {
			kindleCvmStopped = true
			log.Println("[UI] cvm STOPPED (sysv)")
		}
	}

	// SIGSTOP volumd 禁止 USBMS 弹出干扰 (KOReader 同款, sysv+upstart 都做)
	if exec.Command("killall", "-STOP", "volumd").Run() == nil {
		kindleVolumdStopped = true
		log.Println("[UI] volumd STOPPED")
	}
}

// EnableCoexistMode 恢复共存模式的所有改动 (顺序对齐 KOReader, 避免重启)
func EnableCoexistMode() {
	if kindleVolumdStopped {
		_ = exec.Command("killall", "-CONT", "volumd").Run()
		kindleVolumdStopped = false
		log.Println("[UI] volumd CONT")
	}
	if kindleInitType == "sysv" && kindleCvmStopped {
		_ = exec.Command("killall", "-CONT", "cvm").Run()
		kindleCvmStopped = false
		log.Println("[UI] cvm CONT (sysv)")
		// KOReader 对 sysv 会额外 send 139 刷键
		_ = exec.Command("sh", "-c", "echo 'send 139' > /proc/keypad 2>/dev/null; echo 'send 139' > /proc/keypad 2>/dev/null").Run()
	}
	if kindleInitType == "upstart" {
		if kindleStatusbarStopped {
			_ = exec.Command("start", "statusbar").Run()
			kindleStatusbarStopped = false
			log.Println("[UI] statusbar started")
		}
		if kindleAwesomeStopped {
			_ = exec.Command("killall", "-CONT", "awesome").Run()
			kindleAwesomeStopped = false
			log.Println("[UI] awesome CONT")
		} else if kindleCvmStopped {
			_ = exec.Command("killall", "-CONT", "cvm").Run()
			kindleCvmStopped = false
			log.Println("[UI] cvm CONT (upstart fallback)")
		}
		if kindlePillowHardDisabled {
			restoreFB()
			_ = exec.Command("lipc-set-prop", "com.lab126.pillow", "disableEnablePillow", "enable").Run()
			_ = exec.Command("lipc-set-prop", "com.lab126.appmgrd", "start", "app://com.lab126.booklet.home").Run()
			kindlePillowHardDisabled = false
			log.Println("[UI] pillow re-enabled")
		} else if kindleFWVersion != "" && !versionGE(kindleFWVersion, "5.6.5") {
			restoreFB()
			_ = exec.Command("lipc-set-prop", "com.lab126.pillow", "interrogatePillow", `{"pillowId": "default_status_bar", "function": "nativeBridge.showMe();"}`).Run()
			_ = exec.Command("lipc-set-prop", "com.lab126.appmgrd", "start", "app://com.lab126.booklet.home").Run()
			log.Println("[UI] status bar soft-restored")
		}
		if kindleWmctrlUsed && kindleTitlebarGeom != "" {
			time.Sleep(250 * time.Millisecond)
			restored := false
			for i := 0; i < 20; i++ {
				_ = exec.Command("sh", "-c", wmctrlCmd("-r", ":titleBar_ID:", "-e", kindleTitlebarGeom)).Run()
				time.Sleep(250 * time.Millisecond)
				cur := getTitlebarGeometry()
				if cur == kindleTitlebarGeom {
					restored = true
					break
				}
			}
			if restored {
				log.Printf("[UI] titleBar restored %s", kindleTitlebarGeom)
			} else {
				log.Printf("[UI] titleBar restore attempted %s (may need reboot)", kindleTitlebarGeom)
			}
			kindleWmctrlUsed = false
		}
	}
	_ = exec.Command("lipc-set-prop", "-i", "com.lab126.powerd", "preventScreenSaver", "0").Run()
	log.Println("[UI] preventScreenSaver=0 restored")
}

func getTitlebarGeometry() string {
	// 查找 wmctrl 二进制 (KOReader 自带, 我们兼容多路径)
	wm := findWmctrl()
	if wm == "" {
		return ""
	}
	out, err := exec.Command("sh", "-c", wm+" -l -G 2>/dev/null | grep ':titleBar_ID:' | awk '{print $2\",\"$3\",\"$4\",\"$5\",\"$6}'").Output()
	if err != nil {
		return ""
	}
	geom := strings.TrimSpace(string(out))
	// 去掉最后的 ,1 保留前5段
	if strings.Count(geom, ",") == 4 {
		return geom
	}
	return geom
}

func findWmctrl() string {
	for _, p := range []string{"/mnt/us/extensions/kkanpan/bin/wmctrl", "/usr/bin/wmctrl", "./wmctrl", "wmctrl"} {
		if _, err := exec.Command("sh", "-c", "test -x "+p).Output(); err == nil {
			// test -x 成功返回0
			return p
		}
		// 更直接用 stat
		if out, _ := exec.Command("sh", "-c", "command -v "+p+" 2>/dev/null").Output(); len(strings.TrimSpace(string(out))) > 0 {
			return strings.TrimSpace(string(out))
		}
	}
	return ""
}

func wmctrlCmd(args ...string) string {
	wm := findWmctrl()
	if wm == "" {
		return "false"
	}
	// 构造带引号的命令
	quoted := wm
	for _, a := range args {
		quoted += " '" + strings.ReplaceAll(a, "'", "'\\''") + "'"
	}
	return quoted
}


