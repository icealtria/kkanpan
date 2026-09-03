package main

import "testing"

func FuzzParseQT(f *testing.F) {
	f.Add("sh600519", "1~贵州茅台~600519~1800.00~1790.00")
	f.Add("hf_GC", "2000.50~10.00")
	f.Add("sh600519", "")
	f.Add("usAAPL", "~0~0~150.25~149.80~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0~0.45~0.30")

	f.Fuzz(func(t *testing.T, code string, raw string) {
		var vals []string
		if raw != "" {
			if contains := raw; contains != "" {
				vals = splitTilde(contains)
			}
		}
		price, change, pct, prev := parseQT(code, vals)
		_ = price
		_ = change
		_ = pct
		_ = prev
	})
}

func splitTilde(s string) []string {
	if s == "" {
		return nil
	}
	result := []string{}
	start := 0
	for i := 0; i <= len(s); i++ {
		if i == len(s) || s[i] == '~' {
			result = append(result, s[start:i])
			start = i + 1
		}
	}
	return result
}

func FuzzParseGtimgRows(f *testing.F) {
	f.Add("09:30 12.50")
	f.Add("single row")
	f.Add("")
	f.Add("   ")
	f.Add("abc def")
	f.Add("09:30 NaN")

	f.Fuzz(func(t *testing.T, input string) {
		_ = parseGtimgRows([]string{input})
	})
}

func FuzzParseHM(f *testing.F) {
	f.Add("09:30")
	f.Add("00:00")
	f.Add("23:59")
	f.Add("invalid")
	f.Add("")
	f.Add(":::")
	f.Add("99:99")
	f.Add("-1:00")
	f.Add("12")
	f.Add("12:30:45")

	f.Fuzz(func(t *testing.T, s string) {
		_ = parseHM(s)
	})
}
