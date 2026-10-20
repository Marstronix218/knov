# Who Knov is for, and the simulated customers

Knov finds revenue a small service business is about to miss: extra work that
was never billed, promises to clients that slipped, and accepted milestones that
nobody invoiced. This document describes the customers that profile implies and
the ten fictional businesses in the simulation that stand in for them. The data
lives in
[`simulation_businesses.json`](../apps/desktop/src-tauri/src/revenue/simulation_businesses.json)
and is loaded from **Simulation** in the desktop app.

## Customer profile

**Who they are.** Owner-led service businesses with 2 to 12 people that sell
project or retainer work to other businesses: agencies, studios, consultancies,
development shops, bookkeepers, architects, managed IT providers, translators.
The owner still sells, delivers, and invoices. There is no finance person and no
contract manager; the "system" is the owner's inbox and memory.

**Why they lose money.** Work is agreed in writing (a statement of work, a
package, a retainer letter) but changed in conversation (Slack, email, calls).
The team is helpful and starts on whatever the client asks for. Invoicing
happens after the fact, from memory, usually at night or at month end. Each
leak is small and embarrassing, so nobody looks for it.

**What they already use.** Gmail or Google Workspace for client email, Slack
(often Slack Connect) for day-to-day client chatter, an accounting tool
(QuickBooks, Xero, FreshBooks, Wave, Bill.com) for invoices, and a handful of
delivery tools on the Mac (Figma, VS Code, Revit, Premiere, memoQ, Google Docs).
They will not adopt a CRM or a PSA tool; Knov has to read what is already there.

## What their emails look like

Client email carries the commercial signal. Across the ten businesses the
inbox contains the same handful of message types, and the simulation labels
each one the way Knov expects a user to confirm it:

| Kind | What it looks like | Example |
| --- | --- | --- |
| Request | A client asks for something not in the agreement | "Could your team also prepare the landscape drawings?" |
| Approval | A client says yes to a quoted extra, often with the price | "Approved: classroom AV install for USD 3,400." |
| Milestone accepted | A sign-off that triggers a payment | "Secure messaging milestone accepted after UAT." |
| Commitment | The owner promises something by a date | "I'll send the pricing proposal by 2025-10-09." |
| Message | Thanks, feedback, logistics: context but not evidence | "Loved the rough cut. A few small notes attached." |

Slack holds the same kinds of messages but skews toward requests and thanks,
because that is where clients ask casually and where the team says yes.

## What their activity data looks like

Knov's Mac collector sees which apps and documents the team works in and for
how long. For these customers that produces evidence of delivery: hours in
Premiere on a drone opener, a GitHub thread for a Plaid integration, Revit
sheets for a landscape package, a memoQ project for a German catalogue, a sent
proposal in Gmail. In the simulation each activity record carries the apps
used, hours, and a work-thread name. Activity is what turns a request into a
finding: a request alone is a conversation, a request plus work with no
approval is unbilled scope.

Billing data is the last piece. The simulation models an accounting sync as a
list of invoices (number, customer, status, amount) plus a "sync complete"
record per customer. Without that completeness record Knov will only ask you to
verify a missing invoice; with it, Knov sizes the gap.

## What their customers look like

The customers' customers are other small and mid-sized organizations: a
brewery, a dental practice, a credit union, a charter school, a law practice,
a restaurant, a developer, a nonprofit. Three to four active engagements per
business is typical. Each engagement has a written scope with a few explicit
exclusions, a fee, and either a milestone schedule or a monthly retainer. Those
exclusions are what Knov reads to decide whether a request is extra.

## The ten simulated businesses

Every business has three or four engagements, a contract per engagement, a
Gmail inbox, Slack channels, work activity, billing records, a scripted
interview, and a documented set of expected findings. The findings below are
what Knov reports after **Connect everything at once**. Connecting sources in a
different order shows intermediate states, for example an unbilled extra that
gets resolved when the approval email arrives.

| Business | What it is | Currency | Unbilled extra (open) | Late promise | Milestone | Approved extra (no finding) |
| --- | --- | --- | --- | --- | --- | --- |
| Brightline Studio | Web agency, 6 people, Portland | USD | Acme Analytics: third dashboard | Northstar Labs | Atlas Systems: unbilled USD 5,000 | Harbor Goods |
| Lanternwood Creative | Brand and video studio, 3 people, Austin | USD | Copperhead Brewing: drone footage | Hollow Oak Foundation | Riverbend Dental: unbilled USD 4,000 | Hollow Oak same-day edits |
| Gridline Architects | Architecture practice, 8 people, Vancouver | CAD | Okonkwo Residence: landscape drawings | Westgate (resolved: proposal was sent) | Sora Ramen: verify, amount unknown | none |
| Kestrel Advisory | Operations consulting, 2 people, Chicago | USD | Fairway Logistics: implementation support | Bluefin SaaS | Fairway: invoiced (resolved) | Orchard Health training |
| Harborlight Bookkeeping | Bookkeeping and payroll, 4 people, Tampa | USD | Salt & Ember: catch-up bookkeeping | Driftwood Outfitters | Palmetto Law: unbilled USD 2,500 | Driftwood sales tax filings |
| Pinecone Software Co. | Custom software, 10 people, Denver | USD | Summit Credit Union: Plaid integration | Alpine Freight (resolved) | Summit CU: unbilled USD 32,000 | Redwood Health EHR integration |
| Meridian Growth | Digital marketing, 7 people, Atlanta | USD | IronWorks Gyms: paid ads management | Glow Skincare | Beacon Realty: invoiced (resolved) | Glow product photography |
| Northwind Interiors | Interior design, 3 people, London | GBP | The Harringtons: garden room design | Marlow Boutique Hotel | Fig & Thistle: unbilled GBP 4,500; Harringtons invoiced (resolved) | Fig & Thistle signage |
| Summit Peak IT | Managed IT services, 5 people, Salt Lake City | USD | Wasatch Family Medicine: server migration | Canyon Accounting | Alta Charter School: unbilled USD 18,000 | Alta classroom AV |
| Clearwater Localization | Translation, 2 people, Toronto | CAD | Nordic Gear: German translation | Maple Legal Services | Nordic Gear: unbilled CAD 7,200 | Bright Path voice-over |

The set deliberately covers each path through the detector:

- an unbilled extra with the amount unknown (every business),
- a late promise that stays open, and two that Knov resolves on its own because
  work activity shows the proposal was sent (Gridline, Pinecone),
- an accepted milestone that is unbilled and sized, one that is only "verify"
  because the accounting sync does not cover that customer (Gridline), and three
  that Knov resolves because the invoice exists (Kestrel, Meridian, Northwind),
- an approved extra in almost every business, which must produce no finding,
- a request with no work behind it (Summit Peak's security audit), which must
  produce no finding,
- three currencies (USD, CAD, GBP).

A Rust test loads every business and asserts that the findings match the
`expected` list in the JSON exactly, so the dataset cannot silently drift from
the rules.

## The scripted interview

Each business includes a five-question interview on one revenue topic (change
requests, invoicing, follow-ups, or pricing), the suggested quick answers the
owner might tap, and what Knov learns after each answer: goal, trigger, steps,
billing and approval rules, bottlenecks, frequency, and automation ideas. In the
app the interview runs without an AI key: the questions are fixed, every answer
is saved as user-reported evidence, and the "What Knov has learned" panel fills
in as you go. It ends with a completed workflow you can review and edit, exactly
as a real interview would.

Scripted interviews are sandbox records. They never appear in the actual
interview list, Workflows, or Knowledge, and they are deleted when you reset the
sandbox or switch businesses.

## How to test

1. `npm install`, then `npm run dev:desktop`.
2. Open **Simulation** and choose a business.
3. Connect the sources one at a time and read the "What Knov found" changes
   after each step, or use **Connect everything at once**.
4. Open **What to look for** under the findings to check them against the
   expected list.
5. Start the **Interview** step. It opens the AI interview page in simulation
   mode. Answer with the suggested replies or your own words, skip a question,
   pause and resume, finish, then **Review & edit** the workflow.
6. Go to **Home**, prepare a change order, invoice, or follow-up draft, answer
   the clarification on **Opportunities**, approve the draft, and record an
   outcome.
7. Choose another business. Switching clears the previous sandbox, and your
   actual data is never touched.

## What the dataset does not cover

Hourly billing with timesheets, retainers with hour banks, partial payments and
refunds, subcontractor costs, and non-English correspondence are not modelled.
Detection still relies on a documented exclusion in the agreement to call a
request "extra"; a request that is merely outside the scope wording, without an
exclusion line, is not raised.
