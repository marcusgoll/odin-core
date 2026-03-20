"""Budget display with spend bar."""
from textual.widgets import Static


class BudgetPanel(Static):
    DEFAULT_CSS = """
    BudgetPanel {
        height: 2;
        padding: 0 1;
    }
    """

    def update_data(self, budgets):
        if not budgets:
            self.update("[dim]No budget data[/dim]")
            return
        daily = budgets.get("daily", {})
        limits = budgets.get("limits", {})
        spend = daily.get("spend_usd", 0) or 0
        limit = limits.get("daily_spend_usd", 50) or 50
        pct = min(spend / limit * 100, 100) if limit > 0 else 0
        bar_width = 20
        filled = int(pct / 100 * bar_width)
        bar = "\u2588" * filled + "\u2591" * (bar_width - filled)
        color = "green" if pct < 70 else "yellow" if pct < 90 else "red"
        self.update(f"Budget: [{color}]{bar}[/] ${spend:.2f}/${limit:.0f} ({pct:.0f}%)")
