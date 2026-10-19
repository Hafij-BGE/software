#!/usr/bin/env python3
"""Craft Suite's own templates: documents, spreadsheets and presentations, original work released
under CC0 (public domain dedication). Writes them to templates/{word,sheet,slide}/ and lists
them in templates/own.json; build_index.py adds the thumbnails and makes templates/index.json.

Usage: scripts/templates/make_own.py [templates-dir]

Fonts are ones each app always has, so no template shows a missing-font notice: Word's built-in
Inter, Source Sans 3 and Source Serif 4; the fonts Slide embeds (Montserrat, Poppins, Playfair
Display, Lato, Nunito Sans and others); and Calibri for spreadsheets, as in a blank workbook.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ooxml import Docx, Pptx, Style, Xlsx, col_name  # noqa: E402

OUT = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "../../templates"))
entries = []


def add(file, title, category):
    entries.append({"file": file, "title": title, "category": category, "license": "CC0-1.0",
                    "author": "Craft Suite", "source": "https://github.com/Hafij-BGE/software"})


def path(sub, name):
    d = os.path.join(OUT, sub)
    os.makedirs(d, exist_ok=True)
    return os.path.join(d, name)


# ================================================================ Word

def letter():
    a = "1F4E79"
    d = Docx("Business letter", body_font="Source Sans 3", heading_font="Inter", accent=a, margins=1300)
    d.p([("NORTHWIND STUDIO", {"font": "Inter", "bold": True, "size": 18, "color": a, "spacing": 2})], after=0)
    d.p([("12 Harbour Street · Springfield 40210 · hello@northwind.example · +1 555 0134", {"size": 9.5, "color": "6B7280"})],
        border=("bottom", 12, a, 8), after=28)
    d.p("18 October 2026", after=18)
    d.p("Ms. Jordan Lee\nOperations Manager\nBrightline Logistics\n400 Market Avenue\nRiverton 50312", after=18)
    d.p([("Subject: Proposal for the 2027 delivery partnership", {"bold": True})], after=14)
    d.p("Dear Ms. Lee,")
    d.p("Thank you for meeting with our team last week. As promised, this letter summarises the partnership we "
        "discussed and the next steps we suggest. We are excited about working together on faster, greener deliveries "
        "across the region.")
    d.p("In short, Northwind Studio will design and run the customer notification service, while Brightline keeps "
        "full ownership of routes and drivers. The attached schedule lists the milestones and the costs for each phase.")
    d.p("If the terms suit you, please sign and return a copy by 15 November. I am happy to answer any questions in the "
        "meantime.", after=20)
    d.p("Kind regards,", after=40)
    d.p([("Alex Morgan", {"bold": True}), ("\nManaging Director, Northwind Studio", {"color": "6B7280"})])
    d.save(path("word", "business-letter.docx"))
    add("word/business-letter.docx", "Business letter", "Letters")


def report():
    a = "0F766E"
    d = Docx("Report", body_font="Source Sans 3", heading_font="Inter", accent=a)
    d.p([("ANNUAL REPORT 2026", {"bold": True, "color": a, "size": 10, "spacing": 3})], after=6)
    d.title_("Growing Together")
    d.subtitle("A year of steady progress across our three regions")
    d.p([("Prepared by the Strategy Team · October 2026", {"color": "6B7280", "size": 10})], border=("bottom", 8, "D1D5DB", 6), after=24)
    d.heading("Executive summary")
    d.p("Revenue grew 18% while costs rose only 6%. Customer satisfaction reached its highest level since we began "
        "measuring it, and two new offices opened on schedule.")
    d.bullet([("Revenue: ", {"bold": True}), "$4.8 million, up 18% from last year"])
    d.bullet([("Customers: ", {"bold": True}), "1,240 active accounts, 210 of them new"])
    d.bullet([("Team: ", {"bold": True}), "54 people in three regions"])
    d.heading("Results by region")
    d.table([["Region", "Revenue", "Growth", "Satisfaction"],
             ["North", "$1.9 M", "+22%", "92%"],
             ["Central", "$1.6 M", "+15%", "89%"],
             ["South", "$1.3 M", "+16%", "90%"],
             [("Total", {"bold": True}), ("$4.8 M", {"bold": True}), ("+18%", {"bold": True}), ("90%", {"bold": True})]],
            [2600, 2200, 2200, 2400], header_fill=a, zebra="F0FDFA", aligns=["left", "right", "right", "right"])
    d.heading("What we learned")
    d.heading("Customers value speed", 2)
    d.p("Accounts that received a reply within one hour renewed twice as often. Next year every region will staff an "
        "early and a late shift.")
    d.heading("Small pilots pay off", 2)
    d.p("Four of the six pilots we ran became permanent services. We will keep a small pilot budget in every region.")
    d.heading("Next year's priorities")
    d.number("Open a fourth office in the west.")
    d.number("Halve the time it takes to onboard a new customer.")
    d.number("Publish a yearly sustainability report.")
    d.save(path("word", "report.docx"))
    add("word/report.docx", "Report", "Reports")


def resume():
    a = "4338CA"
    d = Docx("Résumé", body_font="Source Sans 3", heading_font="Inter", accent=a, body_size=10.5, margins=1000)
    d.p([("Sam Rivera", {"font": "Inter", "bold": True, "size": 30, "color": "111827"})], after=0, line=1.0)
    d.p([("Data Analyst", {"font": "Inter", "size": 13, "color": a})], after=6)
    d.p([("sam.rivera@example.com · +1 555 0199 · Portland, OR · linkedin.com/in/samrivera", {"size": 9.5, "color": "4B5563"})],
        border=("bottom", 8, "C7D2FE", 8), after=16)
    d.heading("Profile", 3)
    d.p("Analyst with six years of experience turning messy data into clear decisions. Comfortable with SQL, Python and "
        "dashboards, and with explaining results to people who don't live in spreadsheets.")
    d.heading("Experience", 3)
    for role, org, dates, points in [
        ("Senior Data Analyst", "Cascade Health", "2023 – present",
         ["Built the patient-flow dashboard used daily by 14 clinics.", "Cut monthly reporting time from five days to one."]),
        ("Data Analyst", "Evergreen Retail", "2020 – 2023",
         ["Forecast store demand within 4% accuracy.", "Ran the A/B tests behind a 9% lift in online sales."]),
    ]:
        d.table([[[[(role, {"bold": True, "size": 11}), (f" · {org}", {"color": a})]], [(dates, {"color": "6B7280"})]]],
                [7300, 2600], no_borders=True, cell_pad=20, aligns=["left", "right"])
        for pnt in points:
            d.bullet(pnt)
    d.heading("Education", 3)
    d.table([[[[("B.Sc. Statistics", {"bold": True}), (" · University of Oregon", {"color": a})]], [("2016 – 2020", {"color": "6B7280"})]]],
            [7300, 2600], no_borders=True, cell_pad=20, aligns=["left", "right"])
    d.heading("Skills", 3)
    d.table([["Analysis", "SQL · Python (pandas) · R · statistics · forecasting"],
             ["Tools", "Spreadsheets · dashboards · Git · cloud warehouses"],
             ["Languages", "English (native) · Spanish (fluent)"]],
            [2000, 7900], no_borders=True, cell_pad=30, bold_first_col=True)
    d.save(path("word", "resume.docx"))
    add("word/resume.docx", "Résumé", "Personal")


def cover_letter():
    a = "4338CA"
    d = Docx("Cover letter", body_font="Source Sans 3", heading_font="Inter", accent=a, margins=1300)
    d.p([("Sam Rivera", {"font": "Inter", "bold": True, "size": 22, "color": "111827"})], after=0)
    d.p([("sam.rivera@example.com · +1 555 0199 · Portland, OR", {"size": 9.5, "color": "4B5563"})],
        border=("bottom", 8, "C7D2FE", 8), after=24)
    d.p("18 October 2026", after=14)
    d.p("Hiring Team\nRiverbend Analytics\n55 Pine Street, Portland", after=14)
    d.p("Dear Hiring Team,")
    d.p("I am applying for the Lead Analyst role advertised on your website. For six years I have helped health and "
        "retail teams make better decisions with data, and Riverbend's focus on public services is exactly where I want to "
        "use those skills next.")
    d.p("At Cascade Health I built the dashboard that fourteen clinics now use every morning to plan staffing. The "
        "project taught me that a good analysis is one people trust and act on, so I spend as much time with the people "
        "using a report as with the data behind it.")
    d.p("I would welcome the chance to discuss how I could help your team. Thank you for your time and consideration.", after=20)
    d.p("Sincerely,", after=36)
    d.p([("Sam Rivera", {"bold": True})])
    d.save(path("word", "cover-letter.docx"))
    add("word/cover-letter.docx", "Cover letter", "Personal")


def minutes():
    a = "B45309"
    d = Docx("Meeting minutes", body_font="Source Sans 3", heading_font="Inter", accent=a, body_size=10.5)
    d.title_("Meeting minutes", after=4)
    d.p([("Product team · weekly sync", {"color": "6B7280"})], after=14)
    d.table([["Date", "Monday 19 October 2026", "Time", "10:00 – 10:45"],
             ["Location", "Room 3B / video call", "Chair", "Priya Shah"],
             ["Note taker", "Tom Becker", "Next meeting", "26 October 2026"]],
            [1500, 3300, 1500, 3300], fills={(r, c): "FEF3C7" for r in range(3) for c in (0, 2)}, border="FCD34D")
    d.heading("Attendees")
    d.p("Priya Shah, Tom Becker, Lena Ortiz, Marcus Hill, Aiko Tanaka. Apologies: Ravi Kumar.")
    d.heading("Agenda")
    d.number("Release 4.2 status")
    d.number("Customer feedback from the beta")
    d.number("Hiring update")
    d.heading("Discussion")
    d.heading("1. Release 4.2 status", 2)
    d.p("All features are merged. Two bugs remain open; both have fixes in review. Release is still planned for Thursday.")
    d.heading("2. Customer feedback", 2)
    d.p("Beta users like the new search but find the export menu hard to discover. Lena will test a toolbar button.")
    d.heading("Action items")
    d.table([["Action", "Owner", "Due", "Status"],
             ["Merge the two remaining bug fixes", "Marcus", "21 Oct", "In progress"],
             ["Prototype an export button", "Lena", "26 Oct", "Not started"],
             ["Schedule interviews for two roles", "Priya", "23 Oct", "Done"]],
            [4400, 1600, 1400, 2200], header_fill=a, zebra="FFFBEB")
    d.save(path("word", "meeting-minutes.docx"))
    add("word/meeting-minutes.docx", "Meeting minutes", "Business")


def proposal():
    a = "7C2D12"
    d = Docx("Project proposal", body_font="Source Serif 4", heading_font="Inter", accent=a, body_size=10.5)
    d.p([("PROJECT PROPOSAL", {"bold": True, "color": a, "size": 10, "spacing": 3})], after=6)
    d.title_("Community Garden Network")
    d.subtitle("Turning five empty lots into shared gardens by spring 2027")
    d.heading("Summary")
    d.p("We propose a network of five community gardens run by local volunteers, with a small paid coordinator. The "
        "gardens will provide fresh food, outdoor learning for two schools and a meeting place for neighbours.")
    d.heading("Goals")
    d.bullet("Open five gardens with at least 120 plots in total.")
    d.bullet("Run twelve workshops a year on growing food.")
    d.bullet("Give 40% of plots to households on low incomes.")
    d.heading("Timeline")
    d.table([["Phase", "What happens", "When"],
             ["1. Agree", "Leases signed with the city; volunteers recruited", "Nov – Dec 2026"],
             ["2. Build", "Beds, paths, water and tool sheds", "Jan – Mar 2027"],
             ["3. Grow", "Plots assigned; first season and workshops", "Apr – Oct 2027"]],
            [2000, 5300, 2300], header_fill=a, zebra="FFF7ED")
    d.heading("Budget")
    d.table([["Item", "Cost"], ["Materials and tools", "$38,000"], ["Water connections", "$12,500"],
             ["Coordinator (part time, 1 year)", "$24,000"], [("Total", {"bold": True}), ("$74,500", {"bold": True})]],
            [6600, 3000], header_fill=a, aligns=["left", "right"])
    d.heading("Next steps")
    d.p("With the council's approval at its November meeting, we can sign leases before the end of the year.")
    d.save(path("word", "project-proposal.docx"))
    add("word/project-proposal.docx", "Project proposal", "Business")


def paper():
    a = "1E3A8A"
    d = Docx("Research paper", body_font="Source Serif 4", heading_font="Source Sans 3", accent=a, body_size=11)
    d.p([("Effects of Light Cycle on Growth of Basil Seedlings", {"font": "Source Sans 3", "bold": True, "size": 20, "color": "111827"})],
        align="center", after=8)
    d.p([("A. Researcher", {"size": 11}), ("¹", {"size": 8}), (",  B. Colleague", {"size": 11}), ("²", {"size": 8})], align="center", after=2)
    d.p([("¹ Department of Plant Science, Example University   ² Institute of Growth Studies", {"size": 9, "color": "6B7280"})],
        align="center", after=18)
    d.p([("Abstract. ", {"bold": True}),
         "We compared basil seedlings grown under 12, 16 and 20 hours of light a day. Plants under 16 hours grew 31% more "
         "leaf mass than those under 12 hours, while 20 hours gave no further gain. These results suggest a practical "
         "light cycle for indoor growers."], shade="EEF2FF", border=("left", 18, a, 8), after=18)
    d.heading("1. Introduction")
    d.p("Indoor growers often run lights for as long as possible, but longer cycles cost energy and may stress plants. "
        "Few studies measure the gain from each extra hour for common herbs.")
    d.heading("2. Methods")
    d.p("Ninety seedlings were split into three groups of thirty and grown for six weeks at 22 °C. Leaf mass was "
        "weighed every week.")
    d.heading("3. Results")
    d.table([["Light per day", "Leaf mass (g)", "Change"],
             ["12 h", "4.2 ± 0.5", "—"], ["16 h", "5.5 ± 0.6", "+31%"], ["20 h", "5.6 ± 0.7", "+33%"]],
            [3200, 3200, 3200], header_fill=a, zebra="EEF2FF", aligns=["left", "center", "center"])
    d.p([("Table 1. ", {"bold": True, "size": 9.5}), ("Mean leaf mass after six weeks (± standard deviation).", {"size": 9.5, "italic": True})])
    d.heading("4. Discussion")
    d.p("Gains level off after 16 hours. Growers can save about a fifth of their lighting energy with no loss of yield.")
    d.heading("References")
    d.number("Author, A. (2024). Light and herb growth. Journal of Indoor Growing, 12(3), 45–52.")
    d.number("Author, B. & Writer, C. (2025). Energy use in vertical farms. Plant Systems, 8, 101–118.")
    d.save(path("word", "research-paper.docx"))
    add("word/research-paper.docx", "Research paper", "Education")


def flyer():
    a = "9D174D"
    d = Docx("Event flyer", body_font="Source Sans 3", heading_font="Source Serif 4", accent=a, page="A4", margins=1100)
    d.p([("YOU'RE INVITED", {"bold": True, "color": a, "size": 12, "spacing": 6})], align="center", before=40, after=10)
    d.p([("Spring Music\nFestival", {"font": "Source Serif 4", "bold": True, "size": 54, "color": "111827"})], align="center",
        line=0.95, after=18)
    d.p([("An afternoon of live music, food stalls and games for the whole family", {"size": 14, "color": "4B5563"})],
        align="center", after=36)
    d.table([[[[("SATURDAY", {"bold": True, "color": a, "size": 10, "spacing": 2})], [("12 April 2027", {"size": 15, "bold": True})]],
              [[("TIME", {"bold": True, "color": a, "size": 10, "spacing": 2})], [("1 pm – 7 pm", {"size": 15, "bold": True})]],
              [[("PLACE", {"bold": True, "color": a, "size": 10, "spacing": 2})], [("Riverside Park", {"size": 15, "bold": True})]]]],
            [3233, 3233, 3234], fills={(0, c): "FCE7F3" for c in range(3)}, border="FBCFE8", cell_pad=220, aligns=["center"] * 3)
    d.p("", after=24)
    d.p([("Free entry · Bring a blanket · Rain or shine", {"size": 13, "color": "4B5563"})], align="center", after=8)
    d.p([("springfest.example", {"bold": True, "color": a, "size": 13})], align="center")
    d.save(path("word", "event-flyer.docx"))
    add("word/event-flyer.docx", "Event flyer", "Events")


# ================================================================ Spreadsheets

def header_styles(accent, font="Calibri"):
    return {
        "title": Style(bold=True, size=20, color="111827", font=font),
        "sub": Style(size=10, color="6B7280", font=font),
        "head": Style(bold=True, color="FFFFFF", fill=accent, font=font, h="center", border=(accent, "all")),
        "cell": Style(font=font, border=("E5E7EB", "all")),
        "money": Style(font=font, num="#,##0.00", border=("E5E7EB", "all")),
        "total": Style(bold=True, font=font, fill="F3F4F6", border=("D1D5DB", "all")),
        "total_money": Style(bold=True, font=font, fill="F3F4F6", num="#,##0.00", border=("D1D5DB", "all")),
        "label": Style(bold=True, color="374151", font=font),
    }


def budget():
    a = "15803D"
    x = Xlsx("Monthly budget")
    s = header_styles(a)
    sh = x.sheet("Budget", tab=a)
    sh.gridlines = False
    sh.widths = {0: 2, 1: 28, 2: 14, 3: 14, 4: 14, 5: 2, 6: 22, 7: 14}
    sh.set(1, 1, "Monthly budget", s["title"])
    sh.set(2, 1, "Fill in the blue cells; everything else adds up by itself.", s["sub"])
    sh.heights[1] = 30
    inp = Style(font="Calibri", num="#,##0.00", fill="EFF6FF", border=("BFDBFE", "all"), color="1E3A8A")
    sh.row(4, 1, ["Income", "Planned", "Actual", "Difference"], s["head"])
    income = [("Salary", 3800, 3800), ("Freelance", 600, 450), ("Other", 100, 160)]
    for i, (name, plan, act) in enumerate(income):
        r = 5 + i
        sh.set(r, 1, name, s["cell"])
        sh.set(r, 2, plan, inp)
        sh.set(r, 3, act, inp)
        sh.set(r, 4, f"=D{r + 1}-C{r + 1}", s["money"])
    ti = 5 + len(income)
    sh.set(ti, 1, "Total income", s["total"])
    for c in (2, 3, 4):
        L = col_name(c)
        sh.set(ti, c, f"=SUM({L}6:{L}{ti})", s["total_money"])
    sh.row(ti + 2, 1, ["Expenses", "Planned", "Actual", "Difference"], s["head"])
    expenses = [("Rent", 1300, 1300), ("Utilities", 180, 205), ("Groceries", 450, 472), ("Transport", 160, 140),
                ("Insurance", 120, 120), ("Phone & internet", 75, 75), ("Eating out", 150, 190), ("Savings", 600, 600),
                ("Other", 100, 64)]
    e0 = ti + 3
    for i, (name, plan, act) in enumerate(expenses):
        r = e0 + i
        sh.set(r, 1, name, s["cell"])
        sh.set(r, 2, plan, inp)
        sh.set(r, 3, act, inp)
        sh.set(r, 4, f"=C{r + 1}-D{r + 1}", s["money"])
    te = e0 + len(expenses)
    sh.set(te, 1, "Total expenses", s["total"])
    for c in (2, 3, 4):
        L = col_name(c)
        sh.set(te, c, f"=SUM({L}{e0 + 1}:{L}{te})", s["total_money"])
    # Summary card.
    card = Style(bold=True, size=11, color="FFFFFF", fill=a, font="Calibri")
    big = Style(bold=True, size=16, color="14532D", fill="DCFCE7", num="#,##0.00", font="Calibri", h="right")
    lab = Style(color="14532D", fill="DCFCE7", font="Calibri")
    sh.set(4, 6, "Summary", card)
    sh.set(4, 7, "", card)
    for i, (label, formula) in enumerate([("Income", f"=D{ti + 1}"), ("Expenses", f"=D{te + 1}"), ("Left over", f"=H6-H7")]):
        sh.set(5 + i, 6, label, lab)
        sh.set(5 + i, 7, formula, big)
        sh.heights[5 + i] = 24
    sh.freeze = (4, 0)
    x.save(path("sheet", "monthly-budget.xlsx"))
    add("sheet/monthly-budget.xlsx", "Monthly budget", "Personal finance")


def invoice():
    a = "1D4ED8"
    x = Xlsx("Invoice", font="Calibri")
    s = header_styles(a, "Calibri")
    sh = x.sheet("Invoice", tab=a)
    sh.gridlines = False
    sh.widths = {0: 2, 1: 40, 2: 10, 3: 14, 4: 16}
    sh.set(1, 1, "INVOICE", Style(bold=True, size=26, color=a, font="Calibri"))
    sh.heights[1] = 38
    sh.set(1, 3, "Invoice no.", Style(color="6B7280", font="Calibri", h="right"))
    sh.set(1, 4, "INV-2026-041", Style(bold=True, font="Calibri", h="right"))
    sh.set(2, 3, "Date", Style(color="6B7280", font="Calibri", h="right"))
    sh.set(2, 4, "18 Oct 2026", Style(font="Calibri", h="right"))
    sh.set(3, 3, "Due", Style(color="6B7280", font="Calibri", h="right"))
    sh.set(3, 4, "17 Nov 2026", Style(font="Calibri", h="right"))
    sh.set(3, 1, "Northwind Studio · 12 Harbour Street, Springfield", s["sub"])
    sh.set(5, 1, "BILL TO", Style(bold=True, size=9, color=a, font="Calibri"))
    sh.set(6, 1, "Brightline Logistics", Style(bold=True, font="Calibri"))
    sh.set(7, 1, "400 Market Avenue, Riverton", Style(font="Calibri", color="4B5563"))
    sh.row(9, 1, ["Description", "Qty", "Unit price", "Amount"], s["head"])
    items = [("Design workshop (half day)", 2, 650), ("Notification service setup", 1, 2400), ("Monthly hosting", 3, 120),
             ("Support hours", 6, 85)]
    for i, (desc, q, price) in enumerate(items):
        r = 10 + i
        sh.set(r, 1, desc, s["cell"])
        sh.set(r, 2, q, Style(font="Calibri", h="center", border=("E5E7EB", "all")))
        sh.set(r, 3, price, s["money"])
        sh.set(r, 4, f"=C{r + 1}*D{r + 1}", s["money"])
    end = 10 + len(items)
    for r in range(end, end + 2):  # room for more lines
        sh.row(r, 1, [None, None, None, f"=C{r + 1}*D{r + 1}"], styles=[s["cell"], s["cell"], s["money"], s["money"]])
    last = end + 2
    lab = Style(font="Calibri", color="374151", h="right")
    sh.set(last + 1, 3, "Subtotal", lab)
    sh.set(last + 1, 4, f"=SUM(E11:E{last})", Style(font="Calibri", num="#,##0.00"))
    sh.set(last + 2, 3, "Tax rate", lab)
    sh.set(last + 2, 4, 0.08, Style(font="Calibri", num="0%", fill="EFF6FF", color="1E3A8A"))
    sh.set(last + 3, 3, "Tax", lab)
    sh.set(last + 3, 4, f"=E{last + 2}*E{last + 3}", Style(font="Calibri", num="#,##0.00"))
    sh.set(last + 4, 3, "Total due", Style(bold=True, font="Calibri", h="right", fill=a, color="FFFFFF"))
    sh.set(last + 4, 4, f"=E{last + 2}+E{last + 4}", Style(bold=True, size=13, font="Calibri", num="#,##0.00", fill=a, color="FFFFFF"))
    sh.heights[last + 4] = 26
    sh.set(last + 6, 1, "Payment: bank transfer to IBAN XX00 0000 0000 0000 · Thank you for your business!", s["sub"])
    x.save(path("sheet", "invoice.xlsx"))
    add("sheet/invoice.xlsx", "Invoice", "Business")


def timesheet():
    a = "7C3AED"
    x = Xlsx("Weekly timesheet")
    s = header_styles(a)
    sh = x.sheet("Timesheet", tab=a)
    sh.gridlines = False
    sh.widths = {0: 2, 1: 14, 2: 12, 3: 12, 4: 12, 5: 12, 6: 30}
    sh.set(1, 1, "Weekly timesheet", s["title"])
    sh.heights[1] = 30
    sh.set(3, 1, "Employee", s["label"])
    sh.set(3, 2, "Jamie Park", Style(font="Calibri"))
    sh.set(4, 1, "Week of", s["label"])
    sh.set(4, 2, "19 Oct 2026", Style(font="Calibri"))
    sh.row(6, 1, ["Day", "Start", "End", "Break (h)", "Hours", "Notes"], s["head"])
    tm = Style(font="Calibri", num="hh:mm", h="center", border=("E5E7EB", "all"), fill="F5F3FF")
    days = [("Monday", "09:00", "17:30", 0.5), ("Tuesday", "08:30", "17:00", 0.5), ("Wednesday", "09:00", "18:00", 1),
            ("Thursday", "09:00", "17:00", 0.5), ("Friday", "09:00", "15:00", 0.5), ("Saturday", "", "", 0), ("Sunday", "", "", 0)]
    for i, (day, st, en, br) in enumerate(days):
        r = 7 + i
        sh.set(r, 1, day, s["cell"])
        sh.set(r, 2, f'=TIMEVALUE("{st}")' if st else None, tm)
        sh.set(r, 3, f'=TIMEVALUE("{en}")' if en else None, tm)
        sh.set(r, 4, br, Style(font="Calibri", num="0.0", h="center", border=("E5E7EB", "all"), fill="F5F3FF"))
        sh.set(r, 5, f"=IF(D{r + 1}=\"\",0,(D{r + 1}-C{r + 1})*24-E{r + 1})", Style(font="Calibri", num="0.00", h="center", border=("E5E7EB", "all")))
        sh.set(r, 6, "", s["cell"])
    t = 7 + len(days)
    sh.set(t, 1, "Total", s["total"])
    sh.fill(t, 2, t, 4, s["total"])
    sh.set(t, 5, f"=SUM(F8:F{t})", Style(bold=True, font="Calibri", num="0.00", h="center", fill="F3F4F6", border=("D1D5DB", "all")))
    sh.set(t, 6, "", s["total"])
    sh.set(t + 2, 1, "Hourly rate", s["label"])
    sh.set(t + 2, 2, 28, Style(font="Calibri", num="#,##0.00", fill="F5F3FF"))
    sh.set(t + 3, 1, "Pay", s["label"])
    sh.set(t + 3, 2, f"=F{t + 1}*C{t + 3}", Style(bold=True, font="Calibri", num="#,##0.00"))
    x.save(path("sheet", "weekly-timesheet.xlsx"))
    add("sheet/weekly-timesheet.xlsx", "Weekly timesheet", "Business")


def tracker():
    a = "0E7490"
    x = Xlsx("Project tracker")
    s = header_styles(a)
    sh = x.sheet("Tasks", tab=a)
    sh.gridlines = False
    sh.widths = {0: 2, 1: 34, 2: 14, 3: 13, 4: 13, 5: 14, 6: 12}
    sh.set(1, 1, "Project tracker", s["title"])
    sh.set(2, 1, "Website relaunch · update Status and % done as work moves.", s["sub"])
    sh.heights[1] = 30
    sh.row(4, 1, ["Task", "Owner", "Start", "Due", "Status", "% done"], s["head"])
    tasks = [("Collect requirements", "Priya", "05 Oct 2026", "09 Oct 2026", "Done", 1),
             ("Design new pages", "Lena", "12 Oct 2026", "23 Oct 2026", "In progress", 0.6),
             ("Write content", "Tom", "19 Oct 2026", "06 Nov 2026", "In progress", 0.3),
             ("Build pages", "Marcus", "26 Oct 2026", "20 Nov 2026", "Not started", 0),
             ("Test on phones", "Aiko", "16 Nov 2026", "25 Nov 2026", "Not started", 0),
             ("Launch", "Priya", "30 Nov 2026", "30 Nov 2026", "Not started", 0)]
    status_fill = {"Done": "DCFCE7", "In progress": "FEF9C3", "Not started": "F3F4F6"}
    for i, (task, owner, start, due, status, pct) in enumerate(tasks):
        r = 5 + i
        sh.set(r, 1, task, s["cell"])
        sh.set(r, 2, owner, s["cell"])
        sh.set(r, 3, start, Style(font="Calibri", h="center", border=("E5E7EB", "all")))
        sh.set(r, 4, due, Style(font="Calibri", h="center", border=("E5E7EB", "all")))
        sh.set(r, 5, status, Style(font="Calibri", h="center", fill=status_fill[status], border=("E5E7EB", "all")))
        sh.set(r, 6, pct, Style(font="Calibri", num="0%", h="center", border=("E5E7EB", "all")))
    n = 5 + len(tasks)
    sh.set(n + 1, 1, "Overall progress", s["label"])
    sh.set(n + 1, 2, f"=AVERAGE(G6:G{n})", Style(bold=True, size=14, color=a, font="Calibri", num="0%"))
    sh.set(n + 2, 1, "Tasks done", s["label"])
    sh.set(n + 2, 2, f'=COUNTIF(F6:F{n},"Done")&" of "&COUNTA(B6:B{n})', Style(font="Calibri"))
    sh.freeze = (5, 0)
    x.save(path("sheet", "project-tracker.xlsx"))
    add("sheet/project-tracker.xlsx", "Project tracker", "Business")


def gradebook():
    a = "BE123C"
    x = Xlsx("Grade book")
    s = header_styles(a)
    sh = x.sheet("Grades", tab=a)
    sh.gridlines = False
    sh.widths = {0: 2, 1: 22, 2: 11, 3: 11, 4: 11, 5: 11, 6: 11, 7: 10}
    sh.set(1, 1, "Grade book", s["title"])
    sh.set(2, 1, "Class 9B · Mathematics · weights in row 6", s["sub"])
    sh.heights[1] = 30
    sh.row(4, 1, ["Student", "Quiz 1", "Quiz 2", "Project", "Exam", "Average", "Grade"], s["head"])
    wt = Style(italic=True, color="6B7280", font="Calibri", num="0%", h="center", fill="FFF1F2")
    sh.set(5, 1, "Weight", Style(italic=True, color="6B7280", font="Calibri", fill="FFF1F2"))
    for c, w in zip(range(2, 6), [0.15, 0.15, 0.3, 0.4]):
        sh.set(5, c, w, wt)
    sh.fill(5, 6, 5, 7, Style(fill="FFF1F2"))
    students = [("Ava Brown", 82, 88, 91, 85), ("Ben Carter", 74, 70, 80, 72), ("Chloe Davis", 95, 92, 98, 94),
                ("Dan Evans", 61, 68, 75, 66), ("Ella Foster", 88, 85, 84, 90), ("Finn Green", 70, 77, 72, 81)]
    num = Style(font="Calibri", h="center", border=("E5E7EB", "all"))
    for i, (name, *marks) in enumerate(students):
        r = 6 + i
        sh.set(r, 1, name, s["cell"])
        for c, m in zip(range(2, 6), marks):
            sh.set(r, c, m, num)
        R = r + 1
        sh.set(r, 6, f"=SUMPRODUCT(C{R}:F{R},$C$6:$F$6)", Style(bold=True, font="Calibri", num="0.0", h="center", border=("E5E7EB", "all")))
        sh.set(r, 7, f'=IF(G{R}>=90,"A",IF(G{R}>=80,"B",IF(G{R}>=70,"C",IF(G{R}>=60,"D","F"))))', num)
    e = 6 + len(students)
    sh.set(e, 1, "Class average", s["total"])
    for c in range(2, 7):
        L = col_name(c)
        sh.set(e, c, f"=AVERAGE({L}7:{L}{e})", Style(bold=True, font="Calibri", num="0.0", h="center", fill="F3F4F6", border=("D1D5DB", "all")))
    sh.set(e, 7, "", s["total"])
    sh.freeze = (6, 2)
    x.save(path("sheet", "grade-book.xlsx"))
    add("sheet/grade-book.xlsx", "Grade book", "Education")


def expenses():
    a = "C2410C"
    x = Xlsx("Expense report")
    s = header_styles(a)
    sh = x.sheet("Expenses", tab=a)
    sh.gridlines = False
    sh.widths = {0: 2, 1: 13, 2: 18, 3: 34, 4: 13, 5: 14}
    sh.set(1, 1, "Expense report", s["title"])
    sh.heights[1] = 30
    sh.set(3, 1, "Name", s["label"])
    sh.set(3, 2, "Jordan Lee", Style(font="Calibri"))
    sh.set(4, 1, "Purpose", s["label"])
    sh.set(4, 2, "Client visit, Riverton", Style(font="Calibri"))
    sh.row(6, 1, ["Date", "Category", "Description", "Receipt", "Amount"], s["head"])
    rows = [("12 Oct 2026", "Travel", "Train tickets, return", "Yes", 86.40), ("12 Oct 2026", "Lodging", "Hotel, 2 nights", "Yes", 238),
            ("13 Oct 2026", "Meals", "Lunch with client", "Yes", 64.25), ("13 Oct 2026", "Transport", "Taxi to client office", "No", 22.8),
            ("14 Oct 2026", "Meals", "Dinner", "Yes", 31.9)]
    for i, (dt, cat, desc, rec, amt) in enumerate(rows):
        r = 7 + i
        sh.row(r, 1, [dt, cat, desc, rec], styles=[s["cell"]] * 4)
        sh.set(r, 5, amt, s["money"])
    e = 7 + len(rows)
    for r in range(e, e + 3):
        sh.row(r, 1, [None, None, None, None, None], styles=[s["cell"]] * 4 + [s["money"]])
    t = e + 3
    sh.set(t, 4, "Total", s["total"])
    sh.set(t, 5, f"=SUM(F8:F{t})", s["total_money"])
    sh.set(t + 2, 1, "By category", s["label"])
    for i, cat in enumerate(["Travel", "Lodging", "Meals", "Transport"]):
        r = t + 3 + i
        sh.set(r, 1, cat, Style(font="Calibri"))
        sh.set(r, 2, f'=SUMIF(C8:C{t},B{r + 1},F8:F{t})', Style(font="Calibri", num="#,##0.00"))
    sh.freeze = (7, 0)
    x.save(path("sheet", "expense-report.xlsx"))
    add("sheet/expense-report.xlsx", "Expense report", "Business")


def lab_log():
    a = "4D7C0F"
    x = Xlsx("Experiment log")
    s = header_styles(a)
    sh = x.sheet("Runs", tab=a)
    sh.gridlines = False
    sh.widths = {0: 2, 1: 10, 2: 13, 3: 16, 4: 12, 5: 12, 6: 12, 7: 30}
    sh.set(1, 1, "Experiment log", s["title"])
    sh.set(2, 1, "One row per run; statistics update below the table.", s["sub"])
    sh.heights[1] = 30
    sh.row(4, 1, ["Run", "Date", "Sample", "Condition", "Replicate", "Value", "Notes"], s["head"])
    data = [(1, "A-01", "Control", 1, 4.21), (2, "A-02", "Control", 2, 4.05), (3, "A-03", "Control", 3, 4.38),
            (4, "B-01", "Treated", 1, 5.62), (5, "B-02", "Treated", 2, 5.31), (6, "B-03", "Treated", 3, 5.78)]
    for i, (run, sample, cond, rep, val) in enumerate(data):
        r = 5 + i
        sh.row(r, 1, [run, f"{12 + i} Oct 2026", sample, cond, rep], styles=[Style(font="Calibri", h="center", border=("E5E7EB", "all"))] * 5)
        sh.set(r, 6, val, Style(font="Calibri", num="0.00", h="center", border=("E5E7EB", "all")))
        sh.set(r, 7, "", s["cell"])
    n = 5 + len(data)
    sh.row(n + 1, 4, ["Condition", "Mean", "SD", "n"], s["head"])
    for i, cond in enumerate(["Control", "Treated"]):
        r = n + 2 + i
        R = r + 1
        st = Style(font="Calibri", num="0.00", h="center", border=("E5E7EB", "all"))
        sh.set(r, 4, cond, s["cell"])
        sh.set(r, 5, f'=AVERAGEIF(E6:E{n},E{R},G6:G{n})', st)
        first = 6 + 3 * i  # each condition's three runs sit together
        sh.set(r, 6, f'=STDEV(G{first}:G{first + 2})', st)
        sh.set(r, 7, f'=COUNTIF(E6:E{n},E{R})', Style(font="Calibri", h="center", border=("E5E7EB", "all")))
    sh.freeze = (5, 0)
    x.save(path("sheet", "experiment-log.xlsx"))
    add("sheet/experiment-log.xlsx", "Experiment log", "Education")


# ================================================================ Presentations

THEMES = [
    # file, title, category, background, panel, text, muted, accent, accent2, heading font, body font
    ("pitch-deck", "Pitch deck", "Business", "0F172A", "1E293B", "F8FAFC", "94A3B8", "F97316", "38BDF8", "Montserrat", "Inter"),
    ("clean-minimal", "Clean minimal", "Business", "FFFFFF", "F3F4F6", "111827", "6B7280", "2563EB", "93C5FD", "Inter", "Inter"),
    ("forest-story", "Forest story", "Creative", "F4F1EA", "E8E2D5", "1F2A24", "5F6B63", "2F5D50", "C08B3F", "Playfair Display", "Lato"),
    ("coral-workshop", "Coral workshop", "Education", "FFF7F2", "FFE4D6", "2B2B2B", "6B6B6B", "E8505B", "F9A26C", "Poppins", "Nunito Sans"),
    ("ocean-report", "Ocean report", "Business", "0B3C5D", "0E4A72", "F1F8FB", "A9C6D6", "3FD0C9", "F2C14E", "Montserrat", "Source Sans 3"),
    ("research-talk", "Research talk", "Education", "FFFFFF", "EEF2FF", "0F172A", "475569", "1D4ED8", "0EA5E9", "Source Sans 3", "Source Serif 4"),
]


def deck(file, title, category, bg, panel, text, muted, accent, accent2, hf, bf):
    colors = {"dk1": text if bg != "FFFFFF" and int(bg[:2], 16) < 0x80 else "111827", "lt1": "FFFFFF", "dk2": "1F2937",
              "lt2": panel, "accent1": accent, "accent2": accent2, "accent3": muted, "accent4": "10B981",
              "accent5": "8B5CF6", "accent6": "EF4444", "hlink": accent, "folHlink": accent2}
    d = Pptx(title, colors, hf, bf)
    W, H = 13.333, 7.5

    def H1(s, x, y, w, h, t, size=40, color=text, align="l", anchor="t"):
        s.text(x, y, w, h, [(t, {"size": size, "bold": True, "font": hf, "color": color, "align": align, "line": 0.95})], anchor=anchor)

    def P(s, x, y, w, h, t, size=18, color=muted, align="l"):
        s.text(x, y, w, h, [(t, {"size": size, "color": color, "font": bf, "align": align, "line": 1.15})])

    def kicker(s, x, y, t):
        s.text(x, y, 6, 0.4, [(t.upper(), {"size": 12, "bold": True, "color": accent, "font": bf, "spacing": 2})])

    # 1. Title
    s = d.slide(bg)
    s.shape(0, 0, 0.35, H, fill=accent)
    s.shape(W - 4.2, -1.2, 5.4, 5.4, fill=accent2, geom="ellipse", alpha=25)
    s.shape(W - 2.6, 4.6, 3.6, 3.6, fill=accent, geom="ellipse", alpha=20)
    kicker(s, 1.0, 2.0, "Craft Suite · 2026")
    H1(s, 1.0, 2.45, 8.6, 2.2, title, size=60)
    P(s, 1.0, 4.55, 7.5, 0.9, "A short subtitle that says what this presentation is about", size=22)
    s.shape(1.0, 5.75, 1.4, 0.07, fill=accent)
    P(s, 1.0, 5.95, 6, 0.5, "Presenter name · Date", size=14)

    # 2. Agenda
    s = d.slide(bg)
    kicker(s, 0.9, 0.7, "Agenda")
    H1(s, 0.9, 1.05, 11, 1.0, "What we'll cover", size=40)
    items = ["Where we are today", "The opportunity", "Our plan", "Timeline and budget", "Questions"]
    for i, it in enumerate(items):
        y = 2.45 + i * 0.86
        s.shape(0.9, y, 0.62, 0.62, fill=accent, geom="ellipse")
        s.text(0.9, y, 0.62, 0.62, [(str(i + 1), {"size": 18, "bold": True, "color": "FFFFFF", "font": hf, "align": "c"})], anchor="ctr", inset=0)
        P(s, 1.8, y + 0.08, 9, 0.6, it, size=22, color=text)

    # 3. Section
    s = d.slide(accent)
    s.shape(W - 5.5, 0, 5.5, H, fill="FFFFFF", alpha=10)
    s.text(0.9, 2.2, 3, 1.2, [("01", {"size": 80, "bold": True, "color": "FFFFFF", "font": hf})])
    H1(s, 0.9, 3.55, 9, 1.4, "The opportunity", size=48, color="FFFFFF")
    P(s, 0.9, 4.75, 8, 0.8, "Why now is the right moment", size=20, color="FFFFFF")

    # 4. Bullets + card
    s = d.slide(bg)
    kicker(s, 0.9, 0.7, "Today")
    H1(s, 0.9, 1.05, 7.4, 1.0, "Three things we know", size=36)
    s.text(0.9, 2.3, 6.9, 4.2, [
        ([("Customers want speed. ", {"bold": True, "color": text}), ("Replies within an hour double renewals.", {})], {"size": 20, "color": muted, "bullet": True, "bullet_color": accent, "space_after": 14}),
        ([("Costs are flat. ", {"bold": True, "color": text}), ("Growth no longer needs new offices.", {})], {"size": 20, "color": muted, "bullet": True, "bullet_color": accent, "space_after": 14}),
        ([("Pilots work. ", {"bold": True, "color": text}), ("Four of six became permanent services.", {})], {"size": 20, "color": muted, "bullet": True, "bullet_color": accent, "space_after": 14}),
    ])
    s.shape(8.5, 1.2, 3.9, 5.2, fill=panel, geom="roundRect", radius=6000)
    s.text(8.85, 1.6, 3.2, 1.2, [("+18%", {"size": 54, "bold": True, "color": accent, "font": hf})])
    P(s, 8.85, 2.75, 3.2, 1.0, "revenue growth this year", size=16)
    s.shape(8.85, 3.85, 3.2, 0.03, fill=muted, alpha=40)
    s.text(8.85, 4.05, 3.2, 1.2, [("1,240", {"size": 40, "bold": True, "color": text, "font": hf})])
    P(s, 8.85, 4.95, 3.2, 1.0, "active customers", size=16)

    # 5. Bar chart from shapes
    s = d.slide(bg)
    kicker(s, 0.9, 0.7, "Results")
    H1(s, 0.9, 1.05, 11, 1.0, "Growth by quarter", size=36)
    values = [("Q1", 42), ("Q2", 55), ("Q3", 68), ("Q4", 86)]
    base_y, top = 6.4, 2.4
    for i, (q, v) in enumerate(values):
        x = 1.4 + i * 2.6
        h = (base_y - top) * v / 100
        s.shape(x, base_y - h, 1.5, h, fill=accent if i == len(values) - 1 else accent2, geom="roundRect", radius=8000)
        s.text(x - 0.2, base_y - h - 0.55, 1.9, 0.5, [(f"{v}k", {"size": 16, "bold": True, "color": text, "align": "c"})])
        s.text(x - 0.2, base_y + 0.08, 1.9, 0.45, [(q, {"size": 15, "color": muted, "align": "c"})])
    s.shape(1.1, base_y, 10.6, 0.03, fill=muted, alpha=50)

    # 6. Timeline
    s = d.slide(bg)
    kicker(s, 0.9, 0.7, "Plan")
    H1(s, 0.9, 1.05, 11, 1.0, "Timeline", size=36)
    s.shape(1.2, 3.6, 10.9, 0.06, fill=muted, alpha=40)
    for i, (when, what) in enumerate([("Nov", "Agree terms"), ("Jan", "Build"), ("Apr", "Launch"), ("Sep", "Review")]):
        x = 1.2 + i * 3.4
        s.shape(x, 3.38, 0.5, 0.5, fill=accent, geom="ellipse")
        s.text(x - 0.4, 2.5, 2.6, 0.6, [(when, {"size": 20, "bold": True, "color": accent, "font": hf})])
        P(s, x - 0.4, 4.15, 2.8, 1.2, what, size=18, color=text)

    # 7. Closing
    s = d.slide(bg)
    s.shape(0, H - 0.35, W, 0.35, fill=accent)
    H1(s, 0.9, 2.3, 11.5, 1.6, "Thank you", size=64, align="c")
    P(s, 0.9, 3.85, 11.5, 0.8, "Questions? hello@example.com", size=22, align="c")
    d.save(path("slide", f"{file}.pptx"))
    add(f"slide/{file}.pptx", title, category)


def main():
    for f in (letter, report, resume, cover_letter, minutes, proposal, paper, flyer,
              budget, invoice, timesheet, tracker, gradebook, expenses, lab_log):
        f()
    for t in THEMES:
        deck(*t)
    with open(os.path.join(OUT, "own.json"), "w", encoding="utf-8") as fh:
        json.dump({"templates": entries}, fh, indent=1, ensure_ascii=False)
        fh.write("\n")
    print(f"{len(entries)} templates in {OUT}")


if __name__ == "__main__":
    main()
