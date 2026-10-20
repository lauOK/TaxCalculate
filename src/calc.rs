//! 个税与个人养老金抵扣的核心计算逻辑。
//!
//! 全部为纯函数，便于单元测试。

/// 个人养老金每年缴费税前扣除上限（元）
pub const PENSION_CAP: f64 = 12000.0;

/// 年度基本减除费用（每月 5000 元 × 12）
pub const BASIC_DEDUCTION: f64 = 60_000.0;

/// 五险一金个人缴存比例（各项均以「缴费基数」为计算依据）。
///
/// 注意：工伤、生育保险由单位全额缴纳，个人不缴，故不列入。
#[derive(Debug, Clone, Copy)]
pub struct SocialInsurance {
    /// 养老保险个人比例（常见 8%）
    pub pension: f64,
    /// 医疗保险个人比例（常见 2%）
    pub medical: f64,
    /// 失业保险个人比例（常见 0.5%）
    pub unemployment: f64,
    /// 住房公积金个人比例（常见 5%~12%）
    pub housing_fund: f64,
    /// 补充公积金个人比例（部分地区 / 单位有，常见 0%~8%）
    pub supplementary_fund: f64,
    /// 缴费基数下限（元/月），如当地社平工资的 60%
    pub base_min: f64,
    /// 缴费基数上限（元/月），如当地社平工资的 3 倍
    pub base_max: f64,
}

impl Default for SocialInsurance {
    fn default() -> Self {
        Self {
            pension: 0.08,
            medical: 0.02,
            unemployment: 0.005,
            housing_fund: 0.07,
            supplementary_fund: 0.0,
            base_min: 3_000.0,
            base_max: 35_000.0,
        }
    }
}

impl SocialInsurance {
    /// 个人合计缴存比例（不含基数封顶因素）。
    pub fn total_rate(&self) -> f64 {
        self.pension + self.medical + self.unemployment + self.housing_fund + self.supplementary_fund
    }

    /// 根据月收入计算实际缴费基数：在 [base_min, base_max] 之间夹取。
    ///
    /// 若月收入低于下限，按下限缴；高于上限，按上限缴。
    pub fn contribution_base(&self, monthly_income: f64) -> f64 {
        if monthly_income <= 0.0 {
            return 0.0;
        }
        monthly_income.clamp(self.base_min, self.base_max)
    }

    /// 每月的个人缴存总额。
    pub fn monthly_contribution(&self, monthly_income: f64) -> f64 {
        self.contribution_base(monthly_income) * self.total_rate()
    }

    /// 每年的个人缴存总额。
    pub fn yearly_contribution(&self, monthly_income: f64) -> f64 {
        self.monthly_contribution(monthly_income) * 12.0
    }
}

/// 由税前月收入推算年度应纳税所得额。
///
/// 公式：
/// `应纳税所得额 = 税前月收入 × 12 − 年度基本减除费用(60000) − 年社保公积金个人缴存额`
///
/// 结果不小于 0。注意个人养老金缴费在后续 `tax_refund` 中另行扣除。
pub fn taxable_income_from_monthly(
    monthly_income: f64,
    si: &SocialInsurance,
) -> f64 {
    let annual_income = monthly_income * 12.0;
    let deduction = BASIC_DEDUCTION + si.yearly_contribution(monthly_income);
    (annual_income - deduction).max(0.0)
}

/// 个税速算表：(级距上限, 税率, 速算扣除数)
/// 依据综合所得年度税率表（居民个人工资薪金等综合所得）。
const BRACKETS: &[(f64, f64, f64)] = &[
    (36_000.0, 0.03, 0.0),
    (144_000.0, 0.10, 2_520.0),
    (300_000.0, 0.20, 16_920.0),
    (420_000.0, 0.25, 31_920.0),
    (660_000.0, 0.30, 52_920.0),
    (960_000.0, 0.35, 85_920.0),
    (f64::INFINITY, 0.45, 181_920.0),
];

/// 计算应纳税所得额对应的个人所得税。
///
/// `taxable` 为应纳税所得额（已扣除 6 万基本减除费用及各项专项扣除后的金额）。
/// 输入为负或零时返回 0。
pub fn income_tax(taxable: f64) -> f64 {
    if taxable <= 0.0 {
        return 0.0;
    }
    for &(limit, rate, deduction) in BRACKETS {
        if taxable <= limit {
            return taxable * rate - deduction;
        }
    }
    // 理论上不会走到这里（最后一档为无穷大）
    0.0
}

/// 计算缴费 `contribution` 元个人养老金后，相比不缴费少缴的个税（即退税）。
///
/// 缴费额会被夹在 [0, 上限] 之间，且不会使应纳税所得额降到 0 以下。
pub fn tax_refund(taxable: f64, contribution: f64) -> f64 {
    let valid = contribution.clamp(0.0, PENSION_CAP).min(taxable.max(0.0));
    let base_tax = income_tax(taxable);
    let after_tax = income_tax(taxable - valid);
    (base_tax - after_tax).max(0.0)
}

/// 缴费在领取环节需缴纳的税（按 `claim_rate` 对缴费本金计税）。
pub fn claim_tax(contribution: f64, claim_rate: f64) -> f64 {
    contribution.clamp(0.0, PENSION_CAP) * claim_rate
}

/// 单年净收益 = 退税 − 领取时缴的税。
///
/// 这是判断缴费是否划算的核心指标。
pub fn net_benefit(taxable: f64, contribution: f64, claim_rate: f64) -> f64 {
    tax_refund(taxable, contribution) - claim_tax(contribution, claim_rate)
}

/// 求解使单年净收益最大的缴费额。
///
/// 由于退税函数在区间内分段线性、边际税率单调递减，
/// 最优解必出现在「边际税率刚好高于领取税率」的临界点。
/// 采用细粒度扫描 + 边界收敛取得数值解。
pub fn optimal_contribution(taxable: f64, claim_rate: f64) -> f64 {
    let cap = PENSION_CAP.min(taxable.max(0.0));
    if cap <= 0.0 {
        return 0.0;
    }

    let mut best_x = 0.0;
    let mut best_v = net_benefit(taxable, 0.0, claim_rate);

    // 步长 10 元扫描，再在最优邻域做精细搜索
    let coarse = 10.0;
    let mut x = coarse;
    while x <= cap {
        let v = net_benefit(taxable, x, claim_rate);
        if v > best_v {
            best_v = v;
            best_x = x;
        }
        x += coarse;
    }
    // 端点检测（cap 本身可能不是 coarse 的整数倍）
    let v_cap = net_benefit(taxable, cap, claim_rate);
    if v_cap > best_v {
        best_v = v_cap;
        best_x = cap;
    }

    // 精细搜索：在 best_x ± 10 的范围内以 0.01 步长逼近
    let lo = (best_x - coarse).max(0.0);
    let hi = (best_x + coarse).min(cap);
    let mut fine = lo;
    while fine <= hi {
        let v = net_benefit(taxable, fine, claim_rate);
        if v > best_v {
            best_v = v;
            best_x = fine;
        }
        fine += 0.01;
    }

    // 金额取整到分
    (best_x * 100.0).round() / 100.0
}

/// 多年度收益测算结果（每一年的明细）。
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct YearRow {
    /// 年份序号（1 起）
    pub year: u32,
    /// 该年缴费额
    pub contribution: f64,
    /// 该年退税
    pub refund: f64,
    /// 该年领取环节的税
    pub claim_tax: f64,
    /// 累计实缴本金
    pub cum_principal: f64,
}

/// 多年度收益测算汇总。
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Projection {
    pub rows: Vec<YearRow>,
    /// 累计实缴本金
    pub total_principal: f64,
    /// 累计退税
    pub total_refund: f64,
    /// 一次性领取时扣除 3% 后到手金额（不计投资收益）
    pub net_claim: f64,
    /// 最终收益 = 领取到手 + 累计退税 − 累计实缴
    pub final_benefit: f64,
}

/// 按年限测算最终收益。
///
/// - `taxable` 每年的应纳税所得额
/// - `contribution` 每年缴费额
/// - `years` 缴纳年限
/// - `claim_rate` 领取环节税率（默认 0.03）
pub fn project(taxable: f64, contribution: f64, years: u32, claim_rate: f64) -> Projection {
    let annual = contribution.clamp(0.0, PENSION_CAP);
    let refund = tax_refund(taxable, annual);
    let claim = claim_tax(annual, claim_rate);

    let mut rows = Vec::with_capacity(years as usize);
    let mut total_principal = 0.0;
    let mut total_refund = 0.0;
    for y in 1..=years {
        total_principal += annual;
        total_refund += refund;
        rows.push(YearRow {
            year: y,
            contribution: annual,
            refund,
            claim_tax: claim,
            cum_principal: total_principal,
        });
    }

    // 一次性领取：本金全额，扣除领取税率（不计投资收益）
    let net_claim = total_principal * (1.0 - claim_rate);
    let final_benefit = net_claim + total_refund - total_principal;

    Projection {
        rows,
        total_principal,
        total_refund,
        net_claim,
        final_benefit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn test_income_tax_zero() {
        assert!(approx(income_tax(0.0), 0.0));
        assert!(approx(income_tax(-100.0), 0.0));
    }

    #[test]
    fn test_income_tax_brackets() {
        // 36000 以内：3%
        assert!(approx(income_tax(30_000.0), 900.0));
        // 跨档：50000 → 50000*0.1 - 2520 = 2480
        assert!(approx(income_tax(50_000.0), 2_480.0));
        // 100000 → 100000*0.1 - 2520 = 7480
        assert!(approx(income_tax(100_000.0), 7_480.0));
        // 200000 → 200000*0.2 - 16920 = 23080
        assert!(approx(income_tax(200_000.0), 23_080.0));
    }

    #[test]
    fn test_refund() {
        // 应纳税所得额 20 万，缴费 12000
        // 原税 23080，缴费后所得额 188000 → 188000*0.2-16920=20680
        // 退税 = 2400
        let r = tax_refund(200_000.0, 12_000.0);
        assert!(approx(r, 2_400.0));
    }

    #[test]
    fn test_refund_low_income() {
        // 应纳税所得额低于缴费额时，最多只能减到 0
        let r = tax_refund(5_000.0, 12_000.0);
        let expected = income_tax(5_000.0); // 5000*0.03=150
        assert!(approx(r, expected));
    }

    #[test]
    fn test_optimal_high_income() {
        // 高收入者边际税率远高于 3%，应缴满上限
        let opt = optimal_contribution(500_000.0, 0.03);
        assert!(approx(opt, 12_000.0));
    }

    #[test]
    fn test_optimal_zero_income() {
        assert!(approx(optimal_contribution(0.0, 0.03), 0.0));
    }

    #[test]
    fn test_optimal_mid_income() {
        // 应纳税所得额 200000，边际税率 20% > 3%，应缴满
        let opt = optimal_contribution(200_000.0, 0.03);
        assert!(approx(opt, 12_000.0));
    }

    #[test]
    fn test_optimal_stops_at_bracket_edge() {
        // 所得额 40000：缴到 4000 时所得额降至 36000（档位边界），
        // 此后边际税率降为 3%，与领取税率持平，净收益不再增加。
        // 最优解落在 [4000, 12000]（该区间净收益恒为 280），不应低于 4000。
        let opt = optimal_contribution(40_000.0, 0.03);
        assert!(opt >= 3_990.0, "opt = {}", opt);
        let v = net_benefit(40_000.0, opt, 0.03);
        assert!(approx(v, 280.0), "net = {}", v);
        // 缴满也不亏，但收益相同
        let v_full = net_benefit(40_000.0, 12_000.0, 0.03);
        assert!(approx(v_full, 280.0), "full net = {}", v_full);
    }

    #[test]
    fn test_optimal_tiny_income() {
        // 应纳税所得额 2000，边际 3%，与领取税率相同，净收益约为 0
        let opt = optimal_contribution(2_000.0, 0.03);
        let v = net_benefit(2_000.0, opt, 0.03);
        assert!(v.abs() < 1.0);
    }

    #[test]
    fn test_project() {
        let p = project(200_000.0, 12_000.0, 10, 0.03);
        assert!(approx(p.total_principal, 120_000.0));
        assert!(approx(p.total_refund, 24_000.0));
        // 领取到手 = 120000 * 0.97 = 116400
        assert!(approx(p.net_claim, 116_400.0));
        // 最终收益 = 116400 + 24000 - 120000 = 20400
        assert!(approx(p.final_benefit, 20_400.0));
        assert_eq!(p.rows.len(), 10);
    }

    // ---- 五险一金与收入换算 ----

    #[test]
    fn test_si_total_rate() {
        let si = SocialInsurance {
            pension: 0.08,
            medical: 0.02,
            unemployment: 0.005,
            housing_fund: 0.07,
            supplementary_fund: 0.05,
            base_min: 3_000.0,
            base_max: 35_000.0,
        };
        assert!(approx(si.total_rate(), 0.225));
    }

    #[test]
    fn test_si_base_clamp() {
        let si = SocialInsurance::default();
        // 正常收入：基数 = 收入
        assert!(approx(si.contribution_base(10_000.0), 10_000.0));
        // 低于下限：按下限
        assert!(approx(si.contribution_base(1_000.0), 3_000.0));
        // 高于上限：按上限
        assert!(approx(si.contribution_base(100_000.0), 35_000.0));
        // 零收入
        assert!(approx(si.contribution_base(0.0), 0.0));
    }

    #[test]
    fn test_si_monthly_contribution() {
        let si = SocialInsurance {
            pension: 0.08,
            medical: 0.02,
            unemployment: 0.005,
            housing_fund: 0.07,
            supplementary_fund: 0.0,
            base_min: 3_000.0,
            base_max: 35_000.0,
        };
        // 基数 10000，比例 17.5% → 1750
        assert!(approx(si.monthly_contribution(10_000.0), 1_750.0));
        // 年缴 = 21000
        assert!(approx(si.yearly_contribution(10_000.0), 21_000.0));
    }

    #[test]
    fn test_taxable_from_monthly() {
        let si = SocialInsurance {
            pension: 0.08,
            medical: 0.02,
            unemployment: 0.005,
            housing_fund: 0.07,
            supplementary_fund: 0.0,
            base_min: 3_000.0,
            base_max: 35_000.0,
        };
        // 月收入 20000：年收入 240000，社保年缴 20000*0.175*12 = 42000
        // 应纳税所得额 = 240000 - 60000 - 42000 = 138000
        let t = taxable_income_from_monthly(20_000.0, &si);
        assert!(approx(t, 138_000.0), "t = {}", t);
    }

    #[test]
    fn test_taxable_from_monthly_capped_base() {
        let si = SocialInsurance {
            pension: 0.08,
            medical: 0.02,
            unemployment: 0.005,
            housing_fund: 0.07,
            supplementary_fund: 0.0,
            base_min: 3_000.0,
            base_max: 35_000.0,
        };
        // 月收入 50000：基数封顶 35000，社保年缴 35000*0.175*12 = 73500
        // 应纳税所得额 = 600000 - 60000 - 73500 = 466500
        let t = taxable_income_from_monthly(50_000.0, &si);
        assert!(approx(t, 466_500.0), "t = {}", t);
    }

    #[test]
    fn test_taxable_from_monthly_low_income() {
        let si = SocialInsurance::default();
        // 低收入：扣除后不应为负
        let t = taxable_income_from_monthly(3_000.0, &si);
        assert!(t >= 0.0, "t = {}", t);
    }

    #[test]
    fn test_item_amounts_sum_to_total() {
        // 分项金额（基数 × 各项比例）之和应等于月缴存总额
        let si = SocialInsurance {
            pension: 0.08,
            medical: 0.02,
            unemployment: 0.005,
            housing_fund: 0.07,
            supplementary_fund: 0.05,
            base_min: 3_000.0,
            base_max: 35_000.0,
        };
        let base = si.contribution_base(10_000.0);
        let sum = base * si.pension
            + base * si.medical
            + base * si.unemployment
            + base * si.housing_fund
            + base * si.supplementary_fund;
        assert!(approx(sum, si.monthly_contribution(10_000.0)));
        assert!(approx(sum, 2_250.0));
    }
}
