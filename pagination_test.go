package main

import "testing"

func TestClampPage(t *testing.T) {
	SetCurrentPage(0)

	tests := []struct {
		total    int
		expected int
	}{
		{0, 0},
		{1, 0},
		{5, 0},
	}
	for _, tt := range tests {
		ResetPage()
		if got := clampPage(tt.total); got != tt.expected {
			t.Errorf("clampPage(%d) = %d, want %d", tt.total, got, tt.expected)
		}
	}
}

func TestClampPageBounds(t *testing.T) {
	SetCurrentPage(10)
	ResetPage()

	// currentPage=0, total=3 -> stays 0
	SetCurrentPage(0)
	if got := clampPage(3); got != 0 {
		t.Errorf("clampPage(3) = %d, want 0", got)
	}

	// currentPage=5, total=3 -> clamped to 2
	SetCurrentPage(5)
	if got := clampPage(3); got != 2 {
		t.Errorf("clampPage(3) with page=5: got %d, want 2", got)
	}

	// currentPage=-1, total=3 -> clamped to 0
	SetCurrentPage(-1)
	if got := clampPage(3); got != 0 {
		t.Errorf("clampPage(3) with page=-1: got %d, want 0", got)
	}
}

func TestNextPage(t *testing.T) {
	ResetPage()

	// Page 0 -> 1 when total > 1
	if !NextPage(3) {
		t.Error("NextPage(3) from 0 should return true")
	}
	if GetCurrentPage() != 1 {
		t.Errorf("expected page 1, got %d", GetCurrentPage())
	}

	// Page 1 -> 2
	if !NextPage(3) {
		t.Error("NextPage(3) from 1 should return true")
	}
	if GetCurrentPage() != 2 {
		t.Errorf("expected page 2, got %d", GetCurrentPage())
	}

	// Page 2 -> no next (total=3)
	if NextPage(3) {
		t.Error("NextPage(3) from 2 should return false")
	}
	if GetCurrentPage() != 2 {
		t.Errorf("page should stay 2, got %d", GetCurrentPage())
	}
}

func TestPrevPage(t *testing.T) {
	SetCurrentPage(2)

	if !PrevPage() {
		t.Error("PrevPage from 2 should return true")
	}
	if GetCurrentPage() != 1 {
		t.Errorf("expected page 1, got %d", GetCurrentPage())
	}

	if !PrevPage() {
		t.Error("PrevPage from 1 should return true")
	}
	if GetCurrentPage() != 0 {
		t.Errorf("expected page 0, got %d", GetCurrentPage())
	}

	// Page 0 -> no prev
	if PrevPage() {
		t.Error("PrevPage from 0 should return false")
	}
	if GetCurrentPage() != 0 {
		t.Errorf("page should stay 0, got %d", GetCurrentPage())
	}
}

func TestResetPage(t *testing.T) {
	SetCurrentPage(5)
	ResetPage()
	if GetCurrentPage() != 0 {
		t.Errorf("ResetPage should set page to 0, got %d", GetCurrentPage())
	}
}
