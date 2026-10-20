//! 个人所得税与个人养老金抵扣计算器 — GUI 主程序。

// Release 构建时隐藏控制台窗口（debug 下保留，便于查看日志）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calc;

use calc::{
    income_tax, net_benefit, optimal_contribution, project, tax_refund,
    taxable_income_from_monthly, SocialInsurance, BASIC_DEDUCTION, PENSION_CAP,
};
use eframe::egui;

/// 输入模式：直接填所得额，或填税前月收入自动换算。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    /// 直接输入应纳税所得额
    Taxable,
    /// 输入税前月收入，扣除五险一金和基本减除后换算
    MonthlyIncome,
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 760.0])
            .with_min_inner_size([820.0, 620.0])
            .with_title("个税与个人养老金抵扣计算器"),
        ..Default::default()
    };
    eframe::run_native(
        "个税与个人养老金抵扣计算器",
        native_options,
        Box::new(|cc| {
            install_chinese_font(&cc.egui_ctx);
            apply_eye_care_theme(&cc.egui_ctx);
            Ok(Box::new(TaxApp::new(cc)))
        }),
    )
}

/// 护眼灰白主题配色。
mod theme {
    use eframe::egui::Color32;

    /// 主背景：偏暖的浅灰
    pub const BG: Color32 = Color32::from_rgb(0xF2, 0xF3, 0xEF);
    /// 面板 / 分组底色
    pub const PANEL: Color32 = Color32::from_rgb(0xE8, 0xEA, 0xE4);
    /// 输入框底色
    pub const FIELD: Color32 = Color32::from_rgb(0xFA, 0xFA, 0xF7);
    /// 主要文字
    pub const TEXT: Color32 = Color32::from_rgb(0x2B, 0x2E, 0x2A);
    /// 次要文字
    pub const TEXT_DIM: Color32 = Color32::from_rgb(0x6B, 0x70, 0x69);
    /// 强调色（柔和的绿）
    pub const ACCENT: Color32 = Color32::from_rgb(0x3A, 0x7D, 0x5C);
    /// 描边
    pub const STROKE: Color32 = Color32::from_rgb(0xC9, 0xCD, 0xC4);
    /// 警示红
    pub const DANGER: Color32 = Color32::from_rgb(0xB5, 0x3A, 0x3A);
}

/// 应用护眼灰白主题：浅灰底 + 深色文字，避免纯黑或纯白带来的视觉疲劳。
fn apply_eye_care_theme(ctx: &egui::Context) {
    use egui::Color32;
    use theme::*;

    let mut visuals = egui::Visuals::light();

    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.extreme_bg_color = FIELD;
    visuals.faint_bg_color = PANEL;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, STROKE);
    visuals.window_rounding = egui::Rounding::same(6.0);

    // 非交互控件
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = BG;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, STROKE);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);

    // 悬停
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0xDD, 0xE1, 0xD8);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xDD, 0xE1, 0xD8);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);

    // 按下
    visuals.widgets.active.bg_fill = Color32::from_rgb(0xD2, 0xD8, 0xCC);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(0xD2, 0xD8, 0xCC);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);

    // 普通态
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(0xE2, 0xE5, 0xDE);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xE2, 0xE5, 0xDE);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, STROKE);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);

    // 选中态 / 强调
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = Color32::from_rgb(0xB5, 0x6A, 0x2A);

    ctx.set_visuals(visuals);

    // 文本样式：适度放大，减轻阅读疲劳
    use egui::{FontFamily, FontId, TextStyle};
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(21.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(14.0, FontFamily::Monospace),
        ),
        (
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.5, FontFamily::Proportional),
        ),
    ]
    .into();
    ctx.set_style(style);
}

/// 加载系统中文字体，避免界面中文显示为方块。
///
/// 依次尝试常见的 Windows 中文字体，取第一个可用的。
fn install_chinese_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
        "C:/Windows/Fonts/simsun.ttc",
        "C:/Windows/Fonts/Deng.ttf",
    ];

    let mut fonts = egui::FontDefinitions::default();
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let name = format!("cn:{}", path);
        fonts
            .font_data
            .insert(name.clone(), egui::FontData::from_owned(bytes).into());
        // 插到最前面，优先用于渲染中文
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, name.clone());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push(name);
        break;
    }
    ctx.set_fonts(fonts);
}

/// 应用状态。
struct TaxApp {
    // ---- 输入 ----
    /// 输入模式
    mode: InputMode,
    /// 应纳税所得额（已扣 6 万及专项扣除）
    taxable: f64,
    /// 税前月收入（月收入模式下使用）
    monthly_income: f64,
    /// 五险一金比例与基数配置
    social: SocialInsurance,
    /// 计划缴纳的个人养老金金额
    contribution: f64,
    /// 缴纳年限
    years: u32,
    /// 领取环节税率
    claim_rate: f64,
    // ---- 输出缓存 ----
    result: Option<CalcResult>,
}

/// 一次计算的全部结果快照。
#[derive(Clone)]
struct CalcResult {
    /// 换算过程展示：税前年收入
    annual_income: f64,
    /// 换算过程展示：年社保公积金个人缴存额
    si_yearly: f64,
    /// 实际参与计税的应纳税所得额
    taxable_used: f64,
    base_tax: f64,
    refund: f64,
    after_tax: f64,
    optimal: f64,
    optimal_net: f64,
    optimal_refund: f64,
    years: u32,
    total_principal: f64,
    total_refund: f64,
    net_claim: f64,
    final_benefit: f64,
    /// (缴费额, 净收益) 曲线采样点，用于绘图
    curve: Vec<(f64, f64)>,
}

impl TaxApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            mode: InputMode::Taxable,
            taxable: 200_000.0,
            monthly_income: 20_000.0,
            social: SocialInsurance::default(),
            contribution: 12_000.0,
            years: 10,
            claim_rate: 0.03,
            result: None,
        }
    }

    /// 根据当前输入模式，计算实际参与计税的应纳税所得额。
    fn effective_taxable(&self) -> f64 {
        match self.mode {
            InputMode::Taxable => self.taxable.max(0.0),
            InputMode::MonthlyIncome => {
                taxable_income_from_monthly(self.monthly_income.max(0.0), &self.social)
            }
        }
    }

    fn compute(&mut self) {
        let taxable = self.effective_taxable();
        let contribution = self.contribution;
        let years = self.years.max(1);
        let claim_rate = self.claim_rate.clamp(0.0, 1.0);

        let base_tax = income_tax(taxable);
        let refund = tax_refund(taxable, contribution);
        let after_tax = base_tax - refund;

        let optimal = optimal_contribution(taxable, claim_rate);
        let optimal_net = net_benefit(taxable, optimal, claim_rate);
        let optimal_refund = tax_refund(taxable, optimal);

        let p = project(taxable, contribution, years, claim_rate);

        // 采样净收益曲线：0..=12000，步长 200
        let cap = PENSION_CAP.min(taxable.max(0.0));
        let mut curve = Vec::new();
        let mut x = 0.0;
        while x <= cap + 0.001 {
            curve.push((x, net_benefit(taxable, x, claim_rate)));
            x += 200.0;
        }
        if cap > 0.0 && curve.last().map(|p| p.0).unwrap_or(0.0) < cap - 0.001 {
            curve.push((cap, net_benefit(taxable, cap, claim_rate)));
        }

        let si_yearly = self.social.yearly_contribution(self.monthly_income.max(0.0));

        self.result = Some(CalcResult {
            annual_income: self.monthly_income.max(0.0) * 12.0,
            si_yearly,
            taxable_used: taxable,
            base_tax,
            refund,
            after_tax,
            optimal,
            optimal_net,
            optimal_refund,
            years,
            total_principal: p.total_principal,
            total_refund: p.total_refund,
            net_claim: p.net_claim,
            final_benefit: p.final_benefit,
            curve,
        });
    }
}

/// 金额格式化：千分位 + 两位小数。
fn money(v: f64) -> String {    let neg = v < 0.0;
    let s = format!("{:.2}", v.abs());
    let (int_part, dec_part) = s.split_once('.').unwrap_or((s.as_str(), "00"));
    let mut out = String::new();
    let bytes = int_part.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    format!(
        "{}¥{}.{}",
        if neg { "-" } else { "" },
        out,
        dec_part
    )
}

/// 比例输入控件：界面按百分数显示，内部存小数（如 0.08 → 显示 8%）。
fn pct_row(ui: &mut egui::Ui, value: &mut f64) {
    let mut pct = *value * 100.0;
    if ui
        .add(
            egui::DragValue::new(&mut pct)
                .speed(0.1)
                .range(0.0..=50.0)
                .suffix(" %")
                .max_decimals(3),
        )
        .changed()
    {
        *value = pct / 100.0;
    }
}

/// 五险一金单行：项目名 | 比例输入 | 月缴金额 | 年缴金额。
///
/// 金额随比例与缴费基数实时计算并展示，便于边调边看。
fn si_item_row(ui: &mut egui::Ui, name: &str, rate: &mut f64, base: f64) {
    ui.label(name);
    pct_row(ui, rate);
    let monthly = base * *rate;
    ui.label(money(monthly));
    ui.label(money(monthly * 12.0));
    ui.end_row();
}

impl eframe::App for TaxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("个人所得税 · 个人养老金抵扣计算器");
            ui.label("支持按税前月收入换算应纳税所得额，计算退税、最优缴费额与长期收益");
            ui.add_space(8.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.ui_inputs(ui);
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);
                self.ui_results(ui);
            });
        });
    }
}

impl TaxApp {
    fn ui_inputs(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.heading("① 输入参数");
            ui.add_space(6.0);
            egui::Grid::new("inputs")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    // ---- 输入模式切换 ----
                    ui.label("收入输入方式");
                    ui.horizontal(|ui| {
                        let mut changed = false;
                        if ui
                            .selectable_label(
                                self.mode == InputMode::Taxable,
                                "直接填应纳税所得额",
                            )
                            .clicked()
                        {
                            self.mode = InputMode::Taxable;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                self.mode == InputMode::MonthlyIncome,
                                "填税前月收入（自动换算）",
                            )
                            .clicked()
                        {
                            self.mode = InputMode::MonthlyIncome;
                            changed = true;
                        }
                        if changed {
                            // 切换模式时清空旧结果，避免误读
                            self.result = None;
                        }
                    });
                    ui.end_row();

                    // ---- 模式相关的输入 ----
                    match self.mode {
                        InputMode::Taxable => {
                            ui.label("应纳税所得额（元）");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::DragValue::new(&mut self.taxable)
                                        .speed(500.0)
                                        .range(0.0..=100_000_000.0)
                                        .max_decimals(2),
                                );
                                ui.label("已扣 6 万及专项扣除后");
                            });
                            ui.end_row();
                        }
                        InputMode::MonthlyIncome => {
                            ui.label("税前月收入（元）");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::DragValue::new(&mut self.monthly_income)
                                        .speed(500.0)
                                        .range(0.0..=10_000_000.0)
                                        .max_decimals(2),
                                );
                                ui.label("税前应发工资，含个人社保公积金");
                            });
                            ui.end_row();

                            // 实时预览换算结果
                            let t = self.effective_taxable();
                            ui.label("⇒ 换算后应纳税所得额");
                            ui.horizontal(|ui| {
                                ui.strong(money(t));
                                ui.label("（已扣 6 万 + 五险一金 + 补充公积金）");
                            });
                            ui.end_row();
                        }
                    }

                    ui.label("计划缴纳个人养老金（元/年）");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.contribution)
                                .speed(100.0)
                                .range(0.0..=PENSION_CAP)
                                .max_decimals(2),
                        );
                        ui.label(format!("上限 {}", money(PENSION_CAP)));
                    });
                    ui.end_row();

                    ui.label("缴纳年限（年）");
                    ui.add(
                        egui::DragValue::new(&mut self.years)
                            .speed(1.0)
                            .range(1..=45),
                    );
                    ui.end_row();

                    ui.label("领取环节税率");
                    ui.horizontal(|ui| {
                        let mut pct = self.claim_rate * 100.0;
                        if ui
                            .add(
                                egui::DragValue::new(&mut pct)
                                    .speed(0.5)
                                    .range(0.0..=45.0)
                                    .suffix(" %")
                                    .max_decimals(2),
                            )
                            .changed()
                        {
                            self.claim_rate = pct / 100.0;
                        }
                        ui.label("（现行政策默认 3%）");
                    });
                    ui.end_row();
                });

            // ---- 五险一金配置面板 ----
            if self.mode == InputMode::MonthlyIncome {
                ui.add_space(8.0);
                ui.collapsing("五险一金 / 补充公积金缴纳比例设置", |ui| {
                    self.ui_social_panel(ui);
                });
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("💰 开始计算").min_size(egui::vec2(120.0, 32.0)))
                    .clicked()
                {
                    self.compute();
                }
                if ui.button("使用最优缴费额").clicked() {
                    let t = self.effective_taxable();
                    let opt = optimal_contribution(t, self.claim_rate);
                    self.contribution = opt;
                    self.compute();
                }
                if ui.button("重置").clicked() {
                    self.mode = InputMode::Taxable;
                    self.taxable = 200_000.0;
                    self.monthly_income = 20_000.0;
                    self.social = SocialInsurance::default();
                    self.contribution = 12_000.0;
                    self.years = 10;
                    self.claim_rate = 0.03;
                    self.result = None;
                }
            });
        });
    }

    /// 五险一金比例与缴费基数配置面板。
    fn ui_social_panel(&mut self, ui: &mut egui::Ui) {
        let base = self.social.contribution_base(self.monthly_income.max(0.0));

        ui.label(
            egui::RichText::new(format!(
                "缴费基数 {}／月（按此基数计算各项金额，随月收入与上下限自动调整）",
                money(base)
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
        ui.add_space(4.0);

        // 列：项目 | 个人比例 | 月缴存 | 年缴存
        egui::Grid::new("social_grid")
            .num_columns(4)
            .spacing([18.0, 8.0])
            .striped(true)
            .show(ui, |ui| {
                // 表头
                ui.strong("项目");
                ui.strong("个人比例");
                ui.strong("月缴存（元）");
                ui.strong("年缴存（元）");
                ui.end_row();

                si_item_row(ui, "养老保险", &mut self.social.pension, base);
                si_item_row(ui, "医疗保险", &mut self.social.medical, base);
                si_item_row(ui, "失业保险", &mut self.social.unemployment, base);
                si_item_row(ui, "住房公积金", &mut self.social.housing_fund, base);
                si_item_row(ui, "补充公积金", &mut self.social.supplementary_fund, base);

                // 合计行
                let total_rate = self.social.total_rate();
                ui.strong("合计");
                ui.strong(format!("{:.3}%", total_rate * 100.0));
                ui.strong(money(base * total_rate));
                ui.strong(money(base * total_rate * 12.0));
                ui.end_row();
            });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        egui::Grid::new("social_base_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("缴费基数下限（元/月）");
                ui.add(
                    egui::DragValue::new(&mut self.social.base_min)
                        .speed(100.0)
                        .range(0.0..=100_000.0)
                        .max_decimals(2),
                );
                ui.end_row();

                ui.label("缴费基数上限（元/月）");
                ui.add(
                    egui::DragValue::new(&mut self.social.base_max)
                        .speed(100.0)
                        .range(0.0..=500_000.0)
                        .max_decimals(2),
                );
                ui.end_row();
            });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let monthly = self.social.monthly_contribution(self.monthly_income.max(0.0));
        ui.label(format!(
            "个人月缴存合计 {}　|　年缴存合计 {}",
            money(monthly),
            money(monthly * 12.0),
        ));
        if self.monthly_income > self.social.base_max {
            ui.colored_label(
                theme::DANGER,
                format!(
                    "提示：月收入超过基数上限，已按上限 {} 计算缴存额。",
                    money(self.social.base_max)
                ),
            );
        }
        ui.label(
            egui::RichText::new("说明：工伤、生育保险由单位缴纳，个人不缴，故不计入。")
                .small()
                .color(theme::TEXT_DIM),
        );
    }

    fn ui_results(&mut self, ui: &mut egui::Ui) {
        let Some(r) = self.result.clone() else {
            ui.label("点击「开始计算」查看结果。");
            return;
        };

        // 月收入模式下，先展示换算过程
        if self.mode == InputMode::MonthlyIncome {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.heading("② 收入换算明细");
                egui::Grid::new("conv_grid")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("税前年收入");
                        ui.label(money(r.annual_income));
                        ui.end_row();

                        ui.label("减：基本减除费用（6 万）");
                        ui.label(format!("− {}", money(BASIC_DEDUCTION)));
                        ui.end_row();

                        ui.label("减：五险一金 + 补充公积金（个人年缴）");
                        ui.label(format!("− {}", money(r.si_yearly)));
                        ui.end_row();

                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        ui.label("= 应纳税所得额");
                        ui.strong(money(r.taxable_used));
                        ui.end_row();
                    });

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "各项五险一金金额见上方「五险一金 / 补充公积金缴纳比例设置」面板。",
                    )
                    .small()
                    .color(theme::TEXT_DIM),
                );
            });
            ui.add_space(12.0);
        }

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.heading("③ 个税与退税");
            egui::Grid::new("tax_grid")
                .num_columns(2)
                .spacing([20.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("未缴养老金时应缴个税");
                    ui.label(money(r.base_tax));
                    ui.end_row();

                    ui.label("缴纳养老金后可退税额");
                    ui.strong(money(r.refund));
                    ui.end_row();

                    ui.label("实缴个税（退税后）");
                    ui.label(money(r.after_tax));
                    ui.end_row();
                });
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.heading("④ 最省税缴费方案");
            ui.add_space(4.0);
            egui::Grid::new("opt_grid")
                .num_columns(2)
                .spacing([20.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("建议缴纳额（净收益最大）");
                    ui.strong(money(r.optimal));
                    ui.end_row();

                    ui.label("该方案下可退税");
                    ui.label(money(r.optimal_refund));
                    ui.end_row();

                    ui.label("该方案单年净收益");
                    ui.label(money(r.optimal_net));
                    ui.end_row();
                });
            ui.add_space(6.0);
            if r.optimal >= PENSION_CAP - 0.01 {
                ui.colored_label(
                    theme::DANGER,
                    "提示：你的边际税率高于领取税率，建议缴满上限最划算。",
                );
            } else if r.optimal <= 0.01 {
                ui.colored_label(
                    theme::TEXT_DIM,
                    "提示：当前收入水平下缴费抵税收益有限，可不必缴满。",
                );
            } else {
                ui.label("提示：已按边际税率与领取税率的平衡点给出建议额。");
            }
        });

        ui.add_space(12.0);

        // 净收益曲线
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.heading("⑤ 缴费额 — 净收益曲线");
            ui.label("横轴：年缴费额（元）　纵轴：单年净收益（元）");
            ui.add_space(4.0);
            let pts: Vec<[f64; 2]> = r.curve.iter().map(|&(x, y)| [x, y]).collect();
            let line = egui_plot::Line::new(egui_plot::PlotPoints::from(pts))
                .color(egui::Color32::from_rgb(0x2E, 0x6F, 0xA8))
                .width(2.5_f32);
            let opt_line = egui_plot::VLine::new(r.optimal)
                .color(egui::Color32::from_rgb(0xC0, 0x5A, 0x3A))
                .width(1.5_f32);
            egui_plot::Plot::new("benefit_plot")
                .height(240.0)
                .allow_drag(true)
                .allow_zoom(true)
                .show_axes(true)
                .include_y(0.0)
                .show_grid(true)
                .show(ui, |plot_ui| {
                    plot_ui.line(line);
                    plot_ui.vline(opt_line);
                });
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.heading(format!("⑥ 最终收益测算（{} 年）", r.years));
            ui.add_space(4.0);
            egui::Grid::new("proj_grid")
                .num_columns(2)
                .spacing([20.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("累计实缴本金");
                    ui.label(money(r.total_principal));
                    ui.end_row();

                    ui.label("累计退税");
                    ui.label(money(r.total_refund));
                    ui.end_row();

                    ui.label(format!(
                        "一次性领取到手（扣 {:.2}%）",
                        self.claim_rate * 100.0
                    ));
                    ui.label(money(r.net_claim));
                    ui.end_row();

                    ui.separator();
                    ui.separator();
                    ui.end_row();

                    ui.label("最终收益（领取 + 退税 − 实缴）");
                    ui.strong(money(r.final_benefit));
                    ui.end_row();
                });
            ui.add_space(6.0);
            ui.colored_label(
                theme::ACCENT,
                "最终收益 = 领取金额 + 累计退税 − 累计实缴（不计投资收益）",
            );
        });
    }
}
