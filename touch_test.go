package main

import (
	"testing"
	"time"
)

func TestParseHMTouch(t *testing.T) {
	tests := []struct {
		s    string
		want int
	}{
		{"09:30", 570},
		{"15:00", 900},
		{"00:00", 0},
		{"12:30", 750},
		{"invalid", 0},
	}
	for _, tt := range tests {
		if got := parseHM(tt.s); got != tt.want {
			t.Errorf("parseHM(%q) = %d, want %d", tt.s, got, tt.want)
		}
	}
}

func TestMatchRule(t *testing.T) {
	loc := time.FixedZone("CST", 8*3600)

	tests := []struct {
		name string
		now  time.Time
		rule AutoRule
		want bool
	}{
		{
			name: "weekday match, time in range",
			now:  time.Date(2026, 9, 1, 10, 0, 0, 0, loc), // Tuesday
			rule: AutoRule{Weekdays: []int{1, 2, 3, 4, 5}, Start: "09:00", End: "15:00"},
			want: true,
		},
		{
			name: "weekday no match",
			now:  time.Date(2026, 9, 5, 10, 0, 0, 0, loc), // Saturday
			rule: AutoRule{Weekdays: []int{1, 2, 3, 4, 5}, Start: "09:00", End: "15:00"},
			want: false,
		},
		{
			name: "time out of range",
			now:  time.Date(2026, 9, 1, 16, 0, 0, 0, loc),
			rule: AutoRule{Weekdays: []int{1, 2, 3, 4, 5}, Start: "09:00", End: "15:00"},
			want: false,
		},
		{
			name: "empty weekdays = every day",
			now:  time.Date(2026, 9, 5, 10, 0, 0, 0, loc), // Saturday
			rule: AutoRule{Weekdays: nil, Start: "09:00", End: "15:00"},
			want: true,
		},
		{
			name: "overnight range: match before midnight",
			now:  time.Date(2026, 9, 1, 23, 0, 0, 0, loc),
			rule: AutoRule{Start: "22:00", End: "06:00"},
			want: true,
		},
		{
			name: "overnight range: match after midnight",
			now:  time.Date(2026, 9, 2, 2, 0, 0, 0, loc),
			rule: AutoRule{Start: "22:00", End: "06:00"},
			want: true,
		},
		{
			name: "overnight range: no match midday",
			now:  time.Date(2026, 9, 1, 12, 0, 0, 0, loc),
			rule: AutoRule{Start: "22:00", End: "06:00"},
			want: false,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := matchRule(tt.now, tt.rule); got != tt.want {
				t.Errorf("matchRule() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestStyleLabel(t *testing.T) {
	tests := []struct {
		m    string
		want string
	}{
		{"large", "L"},
		{"normal", "S"},
		{"anything", "S"},
	}
	for _, tt := range tests {
		if got := StyleLabel(tt.m); got != tt.want {
			t.Errorf("StyleLabel(%q) = %q, want %q", tt.m, got, tt.want)
		}
	}
}
