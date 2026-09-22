# XVARNA 1.0 — برنامه قطعی سه‌فازی تا انتشار عمومی

**وضعیت:** قرارداد اجرایی نهایی، مشتق‌شده از `XVARNA_MASTER_SPEC.md`  
**مبنای فنی:** XVARNA 0.19.0، commit `175f386`، 49 قابلیت در GH1 و GH2  
**هدف غیرقابل تغییر:** پس از عبور کامل از سه فاز این سند، XVARNA 1.0 منتشر می‌شود. فاز چهارم یا milestone مبهم دیگری پیش از انتشار وجود ندارد.

> **یادداشت اجرایی 2026-09-05:** به درخواست مالک محصول، هدف اجرای فعلی ساخت یک بسته‌ی کامل و checksummed برای ارسال به بازبینان است. تست تعاملی، beta واقعی، سخت‌افزار AMD، sign-off مستقل، soak، امضا، DOI/وب‌سایت و GitHub/Yak/publication به بعد از بازخورد همین بسته موکول شده‌اند. بنابراین خروجی فعلی می‌تواند `READY_TO_SEND_FOR_EXTERNAL_REVIEW` باشد، اما طبق قواعد تغییرناکرده‌ی همین سند، Phase 2 یا Phase 3 و انتشار عمومی را `PASS` نمی‌نامیم تا شواهد انسانی/خارجی و عمل انتشار واقعاً انجام شوند.

---

## 1. قواعد اجرای این سند

1. فازها فقط به ترتیب `Phase 1 → Phase 2 → Phase 3` اجرا می‌شوند.
2. هر فاز یک واحد بزرگ تحویل است؛ کارهای خرد داخل آن به‌عنوان «نسخه جدید محصول» اعلام نمی‌شوند.
3. رفتن به فاز بعد فقط وقتی مجاز است که:
   - تمام checkboxهای اجباری فاز سبز باشند؛
   - تمام artifactهای پذیرش تولید شده باشند؛
   - full check از checkout تمیز پاس شود؛
   - هیچ issue با شدت `S0/S1` یا اولویت `P0` باز نباشد؛
   - گزارش پایان فاز نوشته و توسط مالک محصول تأیید شود.
4. شکست، skip، تست دستی انجام‌نشده یا نبود سخت‌افزار با عبارت «تقریباً تمام» پذیرفته نمی‌شود.
5. هر ادعای عمومی فقط از داده خام قابل‌بازتولید همان release استخراج می‌شود. ادعای «بهترین»، «سریع‌ترین» یا «بی‌رقیب» ممنوع است.
6. دامنه 1.0 همین است: تحلیل خورشید/سایه/تابش، daylight سریع و مسیر Radiance، دید/فضا/privacy، attribution، Study/optimization/report. EnergyPlus، HVAC، CFD، سازه، Revit و قابلیت‌های 1.x وارد این سه فاز نمی‌شوند.
7. SDK پیش‌انتشار GH2 ریسک upstream است، نه مجوز حذف تست. نسخه دقیق آن در هر RC pin و ثبت می‌شود.
8. تغییر component ID، schema یا public API پس از پایان Phase 1 فقط برای blocker صحت/امنیت و همراه migration مجاز است.
9. مدرک پذیرش باید داخل repository یا release archive باشد؛ تأیید شفاهی به‌تنهایی کافی نیست.
10. هر فاز دقیقاً یک تصمیم خروجی دارد: `PASS` یا `FAIL`. در حالت `FAIL` فقط blockerهای همان فاز رفع و gate دوباره کامل اجرا می‌شود.

---

## 2. نقطه شروع تأییدشده

موارد زیر ساخته شده‌اند و دوباره از صفر برنامه‌ریزی نمی‌شوند:

- هسته Rust، C ABI و API مدیریت‌شده .NET 8/10؛
- ZAMYAD scene lifecycle، BLAS/TLAS، instancing، static/dynamic delta، refit/rebuild و cache؛
- VAYU مبتنی بر wgpu/WGSL، CPU fallback، chunking، memory budget، telemetry و parity؛
- ZURVAN/HVARE/ASMAN برای زمان، خورشید، سایه، SVF، irradiance و Solar Envelope؛
- DAENA برای Target/Weighted/Green View، 2D/3D Isovist، Intervisibility، Privacy، Corridor، Landmark، Observer Path و attribution/counterfactual؛
- daylight لحظه‌ای، Daylight Factor، sDA/ASE/UDI، optical materials و Radiance export/runner/comparison؛
- VAHMAN/RASHNU برای manifest، batch/resume/checkpoint، Pareto، sensitivity، robust/multi-fidelity، Parquet/glTF/HTML و Evidence Passport؛
- 49 کامپوننت GH1 روی Rhino 8 و Rhino 9؛
- 49 adapter معنایی GH2 روی Rhino 9 با برابری `IoId` و `ComponentGuid`؛
- packageهای Yak/ZIP/CLI/SDK، checksum و release manifest محلی؛
- full check فعلی شامل Rust، .NET، Radiance، NVIDIA/Intel GPU، coverage و runtime load سبز است.

این شواهد baseline هستند، اما جای سه gate نهایی زیر را نمی‌گیرند.

---

# Phase 1 — Product Freeze و پذیرش کامل داخل Rhino/Grasshopper

## هدف

تبدیل 0.19 از engineering release candidate به محصول feature-complete که کاربر واقعی بتواند بدون نوشتن کد، از نصب تا تحلیل، توضیح نتیجه، optimization و report استفاده کند. پایان این فاز یعنی **کد و UX نسخه 1.0 freeze candidate** است.

## 1.1 پذیرش واقعی میزبان‌ها و تمام 49 قابلیت

- [ ] نصب از Yak ساخته‌شده روی یک محیط تمیز Rhino 8 و یک محیط تمیز Rhino 9؛ بدون استفاده از فایل‌های `bin/obj` توسعه.
- [ ] Rhino 8 + GH1: هر 49 کامپوننت place، wire و solve شود.
- [ ] Rhino 9 + GH1: هر 49 کامپوننت place، wire و solve شود.
- [ ] Rhino 9 + GH2: هر 49 کامپوننت place، wire و solve شود.
- [ ] documentهای نمونه در format بومی هر host ذخیره، Rhino بسته، دوباره باز و نتیجه بازتولید شود.
- [ ] Data Tree/Twig، broadcasting، empty input، invalid length و unit conversion برای workflowهای اصلی بررسی شود.
- [ ] update نسخه/SDK GH2 فقط پس از API review و اجرای مجدد کل acceptance انجام شود.
- [ ] package uninstall/reinstall/upgrade و نبود assembly collision بین Rhino 8/9 تأیید شود.

## 1.2 UX بدون کدنویسی و workflowهای کامل

- [ ] هر 6 workflow بخش 9 سند مادر از geometry تا report روی canvas قابل اجرا باشد:
  1. تحلیل آفتاب سایت شهری؛
  2. طراحی پارامتریک سایبان با scene delta؛
  3. Target/Weighted/Green View؛
  4. Intervisibility و حریم خصوصی نما؛
  5. annual daylight سریع و validated با Radiance؛
  6. multi-option Study/Optimization/Evidence/Report.
- [ ] هیچ workflow پایه‌ای در GH2 به نوشتن دستی JSON وابسته نباشد. JSON به‌عنوان advanced/import interface باقی می‌ماند؛ inputهای ساختاریافته، preset یا builder داخلی باید مسیر Basic را پوشش دهند.
- [ ] Basic Mode فقط ورودی‌های ضروری، preset امن، پیام قابل‌فهم و summary فرض‌ها را نشان دهد.
- [ ] Expert Mode همه tolerance/sample/backend/memory/sky/seed/provenance controls را در دسترس بگذارد.
- [ ] نام، nickname، category، ترتیب portها، tooltip، unit و error message تمام 49 قابلیت review شود.
- [ ] component catalog روی 49 شناسه معنایی freeze شود؛ افزودن component صرفاً برای دورزدن UX ناقص ممنوع است.

## 1.3 اجرای تعاملی، preview و تشخیص خطا

- [ ] تمام تحلیل‌های سنگین از scheduler مشترک استفاده کنند و UI Rhino/GH را freeze نکنند.
- [ ] progress مرحله‌ای و درصدی، cancel واقعی، stale-result suppression و retry برای همه jobهای سنگین پذیرفته شود.
- [ ] cancellation در CPU، GPU و Radiance process tree بدون leak یا نتیجه نیمه‌معتبر آزمایش شود.
- [ ] device-loss و کمبود VRAM در Auto mode به fallback صریح CPU برسد؛ در Require-GPU fail-closed باشد.
- [ ] cache hit/miss، scene revision، reuse/refit/rebuild، backend، adapter و fallback روی canvas قابل مشاهده باشد.
- [ ] legend واحد، preview سطح بالا/LOD، selection، sensor inspection و highlight مستقیم occluder در viewport کار کند.
- [ ] نتیجه مربوط به revision قدیمی هرگز بدون برچسب stale نمایش یا export نشود.
- [ ] Diagnostic Bundle با redaction، hardware/driver/version، log، config و error chain تولید شود.

## 1.4 مستندات و مثال‌های محصول

- [ ] documentation انگلیسی برای هر 49 قابلیت: purpose، inputs، outputs، units، method، assumptions، limitations و example.
- [ ] حداقل 12 tutorial تست‌شده، پوشش‌دهنده همه گروه‌های قابلیت، با فایل canvas و داده کوچک همراه.
- [ ] مسیر onboarding «نصب تا اولین تحلیل معتبر» توسط فردی غیر از توسعه‌دهنده در حداکثر 15 دقیقه انجام شود.
- [ ] troubleshooting برای native load، GPU/driver، units، geometry، cache، EPW، Radiance و GH2 نوشته شود.
- [ ] exampleها از package نصب‌شده اجرا شوند، نه از working tree توسعه.

## 1.5 freeze و کیفیت مهندسی

- [ ] P0/P1 traceability audit: هر requirement به test، سند و example متصل باشد.
- [ ] API/ABI/schema/component catalog به‌عنوان `1.0.0-rc.1` freeze candidate ثبت شود.
- [ ] backward-read/migration برای artifactهای پشتیبانی‌شده 0.16–0.19 تست شود.
- [ ] coverage هسته بحرانی حداقل 85% line و 90% branch یا exception مستند و reviewشده داشته باشد.
- [ ] fuzz/parser/FFI/cache/report negative tests و leak/soak روی jobهای طولانی پاس شوند.
- [ ] `eng/check.ps1` از checkout تمیز و package-built binaries پاس شود.

## Artifactهای اجباری Phase 1

- `artifacts/acceptance/phase-1/host-matrix.json`
- `artifacts/acceptance/phase-1/component-49x3.csv`
- `artifacts/acceptance/phase-1/save-reopen-results.json`
- `artifacts/acceptance/phase-1/async-cancel-device-loss.md`
- `artifacts/acceptance/phase-1/onboarding-observation.md`
- `artifacts/acceptance/phase-1/full-check.log`
- `docs/releases/1.0.0-rc.1.md`

## Gate خروج Phase 1

`PASS` فقط وقتی صادر می‌شود که 49/49 در هر سه host matrix سبز، 6/6 workflow کامل، 12/12 tutorial قابل اجرا، onboarding زیر 15 دقیقه، full check سبز و S0/S1/P0 برابر صفر باشد. مالک محصول گزارش را تأیید می‌کند؛ سپس Phase 2 شروع می‌شود.

---

# Phase 2 — Evidence Freeze، سخت‌افزار واقعی و Closed Beta

## هدف

اثبات اینکه محصول فقط روی fixture تحلیلی و سیستم توسعه کار نمی‌کند. پایان این فاز یعنی **feature، performance، روش علمی و UX بر اساس داده واقعی freeze شده‌اند** و release candidate عمومی‌پذیر است.

## 2.1 corpus معماری عمومی و قابل‌بازتولید

- [ ] corpus دارای license روشن و manifest برای حداقل کلاس‌های زیر freeze شود:
  - `S`: حدود 100k triangle و 10k sensor؛
  - `M`: حدود 1M triangle و 100k sensor؛
  - `L`: حدود 5M triangle و 250k sensor؛
  - pathological: thin/coplanar/mirrored/nested/far-origin/mm/km/corrupt inputs.
- [ ] حداقل سه case study واقعی و قابل‌بازتولید انتخاب شود: ساختمان، سایت شهری و مطالعه daylight/view چندگزینه‌ای.
- [ ] هر dataset شامل source/license، hash، unit، conversion، expected workflow و limitation باشد.
- [ ] benchmark با یک فرمان از checkout تمیز raw data، metadata و report تولید کند.

## 2.2 validation علمی و performance مطلق

- [ ] CPU reference در برابر CPU production و portable GPU برای hit/state/distance/identity/aggregate/top-K روی corpus اجرا شود.
- [ ] visibility parity روی queryهای غیرمبهم بیش از 99.9% و تمام اختلاف‌های edge-case طبقه‌بندی شود.
- [ ] Radiance version-pinned روی room/opening، reflectance variants، external shade، courtyard و office case اجرا شود.
- [ ] point-in-time و annual comparison شامل bias، MAE، RMSE، MAPE، max error، percentile و spatial delta map باشد.
- [ ] Fast Path error envelope و موارد عدم استفاده آن شفاف و dataset-specific منتشر شود.
- [ ] cold/warm، median/P95، memory/VRAM، upload/execute/readback، cache و scene-delta timing برای S/M/L ثبت شود.
- [ ] KPI تعاملی P95 زیر 1 ثانیه روی workload مرجع بررسی شود؛ اگر پاس نشد، workload/KPI فقط با RFC شفاف اصلاح می‌شود.
- [ ] نتیجه benchmark حتی اگر GPU در موردی کندتر باشد بدون حذف یا cherry-pick گزارش شود.

## 2.3 ماتریس سخت‌افزار و پایداری

- [ ] CPU-only روی دستگاه بدون GPU قابل‌استفاده اجرا شود.
- [ ] حداقل یک Intel GPU واقعی پاس شود.
- [ ] حداقل یک NVIDIA GPU واقعی پاس شود.
- [ ] حداقل یک AMD GPU واقعی پاس شود.
- [ ] driver، OS، توان/thermal state، backend و نسخه دقیق ثبت شود.
- [ ] device-loss/fallback واقعی یا fault-injection سطح سیستم روی حداقل دو vendor اجرا شود.
- [ ] memory pressure، chunking، cache corruption recovery و long-run leak soak روی workload L پاس شود.

## 2.4 Closed Beta و مطالعه کاربری

- [ ] حداقل 20 tester واقعی شامل دانشجو، طراح محاسباتی و کاربر محیطی وارد beta شوند.
- [ ] حداقل 10 پروژه واقعی workflow موفق از نصب تا report داشته باشند.
- [ ] رضایت استفاده از داده، anonymization و اجازه انتشار case study ثبت شود.
- [ ] crash-free session حداقل 99.5% در dataset ثبت‌شده beta باشد.
- [ ] موفقیت نصب بدون مداخله دستی حداقل 95% باشد.
- [ ] median زمان اولین تحلیل نمونه حداکثر 15 دقیقه باشد.
- [ ] usability study روی taskهای sun blocker، shade edit، massing compare، privacy conflict و decision explanation انجام شود.
- [ ] feedback به issueهای S0–S3 تبدیل شود؛ همه S0/S1/P0 و bugهای correctness رفع و beta regression دوباره اجرا شود.

## 2.5 review مستقل

- [ ] یک reviewer برای GPU/performance گزارش و raw artifacts را sign off کند.
- [ ] یک reviewer daylight/Radiance روش و محدودیت‌ها را sign off کند.
- [ ] یک reviewer UX/onboarding حداقل دو workflow را بدون راهنمای شفاهی توسعه‌دهنده اجرا کند.
- [ ] validation whitepaper و benchmark report به وضعیت public-ready برسند.

## Artifactهای اجباری Phase 2

- `benchmarks/corpus/manifest.json` و dataset/license files
- `artifacts/acceptance/phase-2/hardware-matrix.json`
- `artifacts/acceptance/phase-2/cpu-gpu-parity.parquet`
- `artifacts/acceptance/phase-2/performance-sml.parquet`
- `artifacts/acceptance/phase-2/radiance-validation.parquet`
- `artifacts/acceptance/phase-2/beta-kpis.json`
- `artifacts/acceptance/phase-2/usability-study.md`
- `artifacts/acceptance/phase-2/reviewer-signoff.md`
- سه case study بازتولیدپذیر
- benchmark report و validation whitepaper نهایی

## Gate خروج Phase 2

`PASS` فقط وقتی صادر می‌شود که AMD/Intel/NVIDIA/CPU کامل، corpus S/M/L منتشرشدنی، Radiance validation کامل، 20 tester و 10 پروژه واقعی، KPIهای crash/install/onboarding پاس، سه review مستقل ثبت و S0/S1/P0 برابر صفر باشد. مالک محصول گزارش را تأیید می‌کند؛ سپس Phase 3 شروع می‌شود.

---

# Phase 3 — Release Candidate، پژوهش و انتشار عمومی 1.0

## هدف

بستن supply chain، هویت، documentation، citation و launch؛ اجرای soak نهایی؛ سپس انتشار واقعی XVARNA 1.0. پایان این فاز «آماده انتشار» نیست—**خود انتشار عمومی، آخرین عمل این فاز است**.

## 3.1 امنیت، حقوق و supply chain

- [ ] نام XVARNA و نام موتورهای فرعی از نظر trademark، GitHub organization، domain، package registry و social handles بررسی و تصمیم ثبت شود.
- [ ] Apache-2.0 source، license مستندات/داده و تمام dependency/licenseها audit شود.
- [ ] `SECURITY.md`، disclosure email/process، threat model و security review نهایی شود.
- [ ] SBOM استاندارد برای هر artifact تولید شود.
- [ ] artifactها امضا و SHA-256 آنها از سیستم تمیز دوباره تأیید شود.
- [ ] antivirus false-positive scan و clean-install matrix نهایی پاس شود.
- [ ] rollback، hotfix، نتیجه نادرست و artifact revocation procedure نوشته شود.

## 3.2 repository، documentation و community

- [ ] repository عمومی شامل source، history مناسب، issue templates، PR template، Code of Conduct، CONTRIBUTING، governance و roadmap 1.x باشد.
- [ ] documentation website انگلیسی با search، install، 49 component reference، 12 tutorial، method، limits و troubleshooting deploy شود.
- [ ] landing page، download، compatibility، benchmark، validation، case-study و citation page فعال باشد.
- [ ] GitHub Issues/Discussions، security contact و support policy فعال شوند.
- [ ] تمام linkها، example downloadها و checksumها از یک سیستم خارج از محیط توسعه تست شوند.

## 3.3 خروجی پژوهشی و citation

- [ ] technical report/preprint شامل مسئله، related work، architecture، incremental scene، attribution، Fast/Reference، benchmark، validation، usability و limitations نهایی شود.
- [ ] raw reproducibility package و سه case study با version/hash ثابت archive شود.
- [ ] `CITATION.cff`، preferred citation و BibTeX نهایی شود.
- [ ] software release و dataset/benchmark در Zenodo یا repository معادل archive و DOI دریافت کنند.
- [ ] DOI و نسخه engine/dataset در reportها و وب‌سایت resolve و verify شود.

## 3.4 launch assets

- [ ] visual identity، icon، logo، component screenshots و press kit نهایی شود.
- [ ] hero video 60–90 ثانیه روی scene واقعی، بدون cut فریبنده، شامل تغییر پارامتریک، heatmap update، sensor، occluder highlight، timeline، backend و report ساخته شود.
- [ ] technical deep dive و سه case-study page/video آماده شود.
- [ ] متن GitHub Release، LinkedIn، McNeel Forum و ایمیل استادان/آزمایشگاه‌ها بر اساس claims مجاز نوشته شود.
- [ ] هیچ متن launch شامل ادعای بدون evidence یا مقایسه تبلیغاتی با رقبا نباشد.

## 3.5 RC نهایی و soak

- [ ] version به `1.0.0-rc.1`، code/feature/API/schema/component freeze و release notes نهایی شود.
- [ ] full clean build، full test، benchmark، validation، docs-link و package install از tag candidate پاس شود.
- [ ] RC حداقل 14 روز تقویمی در گروه beta بدون S0/S1، P0، data loss یا correctness regression soak شود.
- [ ] اگر blocker پیدا شد، RC جدید ساخته و دوره 14 روزه از ابتدا اجرا شود.
- [ ] go/no-go record شامل تمام Gateهای 0–8 و KPIهای 1.0 امضا شود.

## 3.6 عمل انتشار—پایان اجباری Phase 3

- [ ] tag immutable با نام `v1.0.0` ساخته و push شود.
- [ ] GitHub Release عمومی با Yak/ZIP/CLI/SDK، SBOM، signature، checksums و release manifest منتشر شود.
- [ ] بسته‌های Rhino 8 و Rhino 9 در Yak/Package Manager عمومی upload و نصب مجدد شوند.
- [ ] documentation/landing page و DOI public شوند.
- [ ] announcementهای آماده‌شده منتشر و support channels پایش شوند.
- [ ] post-release verification از URL و package عمومی انجام و نتیجه ثبت شود.

## Artifactهای اجباری Phase 3

- `artifacts/release/1.0.0/release-manifest.json`
- `artifacts/release/1.0.0/SHA256SUMS.txt`
- SBOM و signature هر artifact
- security/license/trademark review records
- preprint/technical report و DOIها
- website snapshot و broken-link report
- hero video، screenshots و press kit
- `docs/releases/1.0.0.md`
- `artifacts/release/1.0.0/go-no-go.md`
- لینک GitHub Release، Yak packages، documentation و DOI عمومی

## Gate خروج Phase 3

`PASS` فقط پس از soak کامل، go/no-go مثبت، صفر S0/S1/P0، artifactهای امضاشده، DOI فعال، سایت فعال و **انتشار عمومی واقعی `v1.0.0`** صادر می‌شود. اگر upload عمومی انجام نشده باشد، Phase 3 تمام نشده است.

---

## ماتریس نهایی Gateها

| Gate سند مادر | وضعیت ابتدای برنامه | فاز بسته‌شدن قطعی |
|---|---|---|
| Gate 0 — Charter | انجام‌شده | baseline |
| Gate 1 — Reference Correctness | انجام‌شده | regression در همه فازها |
| Gate 2 — CPU Engine | انجام‌شده | regression در همه فازها |
| Gate 3 — Internal Alpha | فنی انجام‌شده؛ پذیرش واقعی ناقص | Phase 1 |
| Gate 4 — GPU Parity | NVIDIA/Intel انجام‌شده؛ corpus/AMD/fault ناقص | Phase 2 |
| Gate 5 — Feature Complete | engine کامل؛ UX/GH2/freeze ناقص | Phase 1 |
| Gate 6 — Closed Beta | انجام‌نشده | Phase 2 |
| Gate 7 — Release Candidate | بخشی از packaging موجود؛ audit/soak/launch ناقص | Phase 3 |
| Gate 8 — Public 1.0 | انجام‌نشده | Phase 3، با انتشار واقعی |

---

## چیزهایی که عمداً بعد از 1.0 می‌مانند

این موارد blocker انتشار نیستند و نباید دوباره برنامه را کش بدهند:

- EnergyPlus/HVAC/whole-building energy؛
- Revit/Dynamo/Blender connector؛
- CFD، سازه، thermal comfort و acoustics؛
- cloud collaboration/SaaS؛
- GPU multi-bounce path tracer؛
- backend اختصاصی NVIDIA؛
- macOS Tier A؛
- optimizerهای GP/qNEHVI/MF-HVKG؛
- تغییر component catalog برای قابلیت‌های 1.x؛
- ترجمه کامل همه UIها؛ مستند فارسی مکمل مجاز است.

---

## فرم تأیید پایان هر فاز

```text
Phase: 1 | 2 | 3
Candidate commit/tag:
Full-check artifact:
Required artifacts complete: YES/NO
Open S0:
Open S1:
Open P0:
Exceptions/RFCs:
Independent reviewer(s):
Product owner decision: PASS/FAIL
Approval date:
Signature/name:
```

هیچ عبارت دیگری جای `PASS` مستند را نمی‌گیرد. پس از `PASS` فاز 3، محصول XVARNA 1.0 منتشر شده است و ادامه کار وارد roadmap نسخه 1.x می‌شود.
