use crate::config::Quote;

#[derive(Debug, Clone)]
pub struct Block {
    pub is_header: bool,
    pub group: String,
    pub quote: Option<Quote>,
    pub h: i32,
    pub gap: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Normal,
    Large,
}

impl Style {
    pub fn label(self) -> &'static str {
        match self {
            Style::Normal => "S",
            Style::Large => "L",
        }
    }
    pub fn card_h(self) -> i32 {
        match self {
            Style::Normal => 103,
            Style::Large => 155,
        }
    }
    pub fn card_gap(self) -> i32 {
        match self {
            Style::Normal => 8,
            Style::Large => 10,
        }
    }
}

fn push_group(blocks: &mut Vec<Block>, group: &str, list: &[Quote], style: Style) {
    if list.is_empty() {
        return;
    }
    blocks.push(Block {
        is_header: true,
        group: group.to_string(),
        quote: None,
        h: 44,
        gap: 6,
    });
    for q in list {
        blocks.push(Block {
            is_header: false,
            group: group.to_string(),
            quote: Some(q.clone()),
            h: style.card_h(),
            gap: style.card_gap(),
        });
    }
}

/// 与 Go buildBlocks 等价, 但去掉了 style 分支的三处复制.
pub fn build_blocks(
    data: &[Quote],
    group_order: &[String],
    effective: &str,
    is_auto: bool,
    matching: &[String],
    style: Style,
) -> Vec<Block> {
    use std::collections::HashMap;
    let mut groups: HashMap<&str, Vec<Quote>> = HashMap::new();
    for d in data {
        groups.entry(d.group.as_str()).or_default().push(d.clone());
    }
    let empty: Vec<Quote> = Vec::new();
    let mut blocks = Vec::new();
    if effective == "ALL" {
        for g in group_order {
            let list = groups.get(g.as_str()).map(|v| v.as_slice()).unwrap_or(&empty);
            push_group(&mut blocks, g, list, style);
        }
        return blocks;
    }
    if is_auto {
        if matching.is_empty() {
            return blocks;
        }
        for g in matching {
            let list = groups.get(g.as_str()).map(|v| v.as_slice()).unwrap_or(&empty);
            push_group(&mut blocks, g, list, style);
        }
        return blocks;
    }
    let list = groups.get(effective).map(|v| v.as_slice()).unwrap_or(&empty);
    push_group(&mut blocks, effective, list, style);
    blocks
}

/// 与 Go paginate 等价 (header 尽量不孤立). 返回每页的 block 切片索引范围.
pub fn paginate(blocks: &[Block], height: i32) -> Vec<(usize, usize)> {
    let mut ph = height - 142 - 70;
    if ph < 200 {
        ph = 200;
    }
    let mut pages = Vec::new();
    let mut start = 0;
    let mut cur_h = 0;
    for (i, b) in blocks.iter().enumerate() {
        // header 在页尾放不下整卡时整体换页 (Go 原逻辑的简化等价版)
        if cur_h > 0 && cur_h + b.h - b.gap > ph {
            pages.push((start, i));
            start = i;
            cur_h = 0;
        }
        cur_h += b.h;
    }
    if start < blocks.len() {
        pages.push((start, blocks.len()));
    }
    if pages.is_empty() {
        pages.push((0, 0));
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(group: &str, code: &str) -> Quote {
        Quote {
            code: code.into(),
            name: code.into(),
            group: group.into(),
            price: 1.0,
            prev: 1.0,
            change: 0.0,
            pct: 0.0,
            prices: vec![],
            timestamps: vec![],
            regular_start: 0,
            regular_end: 0,
            chart_prev_close: 0.0,
        }
    }

    #[test]
    fn single_group_pages() {
        let data = vec![q("g", "a"), q("g", "b")];
        let blocks = build_blocks(&data, &["g".to_string()], "g", false, &[], Style::Normal);
        assert_eq!(blocks.len(), 3); // 1 header + 2 cards
        let pages = paginate(&blocks, 1448);
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn overflow_splits() {
        let data: Vec<Quote> = (0..30).map(|i| q("g", &format!("c{i}"))).collect();
        let blocks = build_blocks(&data, &["g".to_string()], "g", false, &[], Style::Normal);
        let pages = paginate(&blocks, 400);
        assert!(pages.len() > 1);
    }
}
