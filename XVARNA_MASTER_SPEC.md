# XVARNA 1.0

## Master Product, Engineering, Research and Release Specification

> **وضعیت سند:** Baseline Approved — مبنای هنجاری و سند کار پروژه  
> **نسخه سند:** 0.19.0
> **تاریخ مبنا:** 2026-09-02  
> **نام محصول:** XVARNA  
> **نام موقت افزونه:** XVARNA for Grasshopper  
> **مالک محصول و پژوهش:** مؤسس پروژه  
> **زبان سند:** فارسی؛ نام‌های API، کد، فایل و اصطلاحات قراردادی انگلیسی  
> **مجوز پیشنهادی کد:** Apache-2.0  
> **اولین انتشار عمومی هدف:** XVARNA 1.0 Flagship Release

---

## 0. اختیار، کاربرد و شیوه تغییر این سند

این فایل «منبع واحد حقیقت» یا **Single Source of Truth** پروژه XVARNA است. تصمیم‌های مربوط به محصول، پژوهش، معماری نرم‌افزار، محدوده نسخه 1.0، UX، تست، benchmark، انتشار و بازاریابی باید با این سند سازگار باشند. اگر کد، issue، طراحی UI، README یا گفت‌وگویی با این سند تناقض داشت، تا زمانی که این سند رسماً اصلاح نشده است، این سند مرجع است.

این سند هم‌زمان نقش موارد زیر را دارد:

- Product Requirements Document یا PRD؛
- Software Requirements Specification یا SRS؛
- معماری سطح کلان و قرارداد میان زیرسیستم‌ها؛
- برنامه پژوهشی و نقشه مقاله؛
- برنامه QA، validation و benchmark؛
- تعریف انتشار عمومی 1.0؛
- backlog مادر و Definition of Done؛
- ابزار جلوگیری از scope creep و ادعاهای اثبات‌نشده.

### 0.1 واژگان الزام

واژگان زیر در این سند معنای قراردادی دارند:

- **MUST / باید:** بدون آن، نسخه 1.0 قابل انتشار نیست.
- **MUST NOT / نباید:** نقض آن مانع انتشار است.
- **SHOULD / بهتر است:** فقط با دلیل ثبت‌شده می‌توان از آن صرف‌نظر کرد.
- **MAY / می‌تواند:** اختیاری و خارج از release gate اصلی است.
- **P0:** الزام حیاتی و مسدودکننده انتشار.
- **P1:** الزام اصلی نسخه 1.0؛ حذف آن نیازمند تغییر رسمی محدوده است.
- **P2:** قابلیت تکمیلی که در صورت ریسک زمان می‌تواند به 1.x منتقل شود.
- **Experimental:** قابلیت موجود اما فاقد تضمین پایداری API یا دقت پژوهشی.

### 0.2 کنترل تغییرات

هر تغییر عمده باید در یک RFC کوتاه ثبت شود و حداقل شامل این موارد باشد:

1. مسئله؛
2. تصمیم پیشنهادی؛
3. گزینه‌های ردشده؛
4. پیامد فنی و زمانی؛
5. تأثیر بر معیارهای پذیرش؛
6. مهاجرت داده یا API؛
7. شماره نسخه سند.

تغییرات breaking پس از تثبیت API 1.0 تنها در نسخه major بعدی مجازند. تصمیم‌هایی که در بخش «تصمیم‌های قفل‌شده» آمده‌اند بدون RFC تغییر نمی‌کنند.

### 0.3 سیاست انتشار عمومی

پروژه **MVP عمومی کم‌امکانات منتشر نمی‌کند**. توسعه با milestoneهای داخلی، nightly build، Closed Alpha و Closed Beta انجام می‌شود، اما اولین معرفی عمومی محصول باید نسخه پرچم‌دار 1.0 باشد. وجود build داخلی یا beta دعوتی با این سیاست تناقض ندارد.

---

## 1. خلاصه اجرایی

XVARNA یک پلتفرم متن‌باز، سریع، توضیح‌پذیر و چندسکویی برای **تحلیل محیطی و فضایی محاسباتی در معماری** است. محصول در نسخه 1.0 روی تحلیل‌های زیر تمرکز دارد:

- نور مستقیم خورشید و سایه؛
- آسمان، تابش و پتانسیل خورشیدی؛
- daylight اولیه و metricهای سالانه؛
- دید، کیفیت دید، isovist و visibility graph؛
- حریم خصوصی و intervisibility؛
- attribution یا تعیین عامل ایجاد نتیجه؛
- مقایسه سناریو، sensitivity و اتصال به optimization؛
- اجرای تعاملی روی CPU، GPU و در صورت امکان WebGPU؛
- اعتبارسنجی با مسائل تحلیلی و موتورهای مرجع.

هسته محاسباتی عمدتاً با Rust نوشته می‌شود. افزونه Grasshopper یک لایه نازک C# است که از طریق C ABI به هسته native متصل می‌شود. backend قابل‌حمل GPU با `wgpu/WGSL` ساخته می‌شود؛ backend تخصصی NVIDIA بر پایه CUDA/OptiX فقط در صورت اثبات مزیت و امکان نگهداری اضافه خواهد شد. CLI، API و نسخه وب از همان مدل داده و همان هسته مفهومی استفاده می‌کنند.

ویژگی امضادار XVARNA فقط تولید heatmap نیست. سامانه باید بتواند توضیح دهد **کدام شیء، در چه زمانی، با چه سهمی و با چه درجه اطمینانی** موجب افت نور، دید یا حریم خصوصی شده است. این قابلیت با scene identity پایدار، first-hit attribution، counterfactual analysis و گزارش قابل ردیابی پیاده می‌شود.

### 1.1 گزاره محصول

> XVARNA به معمار و طراح محاسباتی اجازه می‌دهد اثر نور، زمان، دید و انسداد را هم‌زمان با تغییر طرح ببیند، علت نتیجه را بفهمد و تصمیم طراحی را با شواهد قابل بازتولید دفاع کند.

### 1.2 نتیجه مطلوب برای کاربر

کاربر باید بتواند یک مدل سایت یا ساختمان را به صحنه XVARNA تبدیل کند، هزاران یا میلیون‌ها پرس‌وجوی فضایی اجرا کند، نتیجه را در viewport ببیند، مانع مؤثر را انتخاب کند، گزینه‌های طراحی را مقایسه کند و یک گزارش قابل استناد بسازد؛ بدون اینکه میان چند افزونه، چند فرمت و چند UI پراکنده رفت‌وآمد کند.

### 1.3 مرز ادعای نسخه 1.0

XVARNA 1.0 باید بهترین تجربه یکپارچه برای **تحلیل بلادرنگ ray-based و early-stage environmental/spatial design** ارائه کند. نسخه 1.0 جایگزین کامل موتورهای HVAC، CFD، انتقال حرارت و شبیه‌سازی انرژی ساختمان نیست. این موتورها حوزه‌های مستقل و چندساله‌اند. XVARNA می‌تواند برای validation یا interoperability به Radiance، EnergyPlus/OpenStudio یا موتورهای دیگر متصل شود، اما نباید بدون شواهد ادعا کند که تمام فیزیک آن‌ها را بازنویسی یا جایگزین کرده است.

---

## 2. هویت، نام و معماری برند

### 2.1 نام مادر: XVARNA

`XVARNA` صورت برندشده‌ای از مفهوم ایرانی باستان `xvarənah` است که معمولاً به شکوه، فَرّ، فروغ و نیروی درخشان تعبیر می‌شود. این نام:

- ریشه فرهنگی مشخص دارد؛
- با نور، توان، شکوه و دیده‌شدن پیوند معنایی دارد؛
- محصول را به یک تحلیل خاص محدود نمی‌کند؛
- از نظر بصری برای لوگو و هویت بین‌المللی متمایز است؛
- امکان نام‌گذاری منسجم زیرسیستم‌ها را فراهم می‌کند.

پیش از انتشار عمومی باید بررسی رسمی trademark، نام سازمان GitHub، دامنه‌های اصلی، package registryها و شبکه‌های اجتماعی انجام شود. بررسی اولیه اینترنتی جایگزین جست‌وجوی حقوقی نیست.

### 2.2 نام زیرسیستم‌ها

| نام | نقش | وضعیت در 1.0 |
|---|---|---|
| `XVARNA Core` | geometry، scene، query، cache و API | P0 |
| `ZAMYAD` | scene compilation، BLAS/TLAS، ray query و spatial attribution | P0 |
| `HVARE` | خورشید، مسیر خورشید و تابش مستقیم | P0 |
| `ASMAN` | آسمان، Sky View، Shadow Mask، diffuse sky و daylight sky | P0 |
| `MEHR` | تابش سالانه EPW/Perez، انرژی صفحه و attribution اتلاف | P0 |
| `ZURVAN` | زمان، timestep، annual schedules و سناریوهای زمانی | P0 |
| `RASHNU` | validation، uncertainty، benchmark و audit | P0 |
| `XVARNA Sight` | دید، isovist، حریم خصوصی و visibility graph | P0 |
| `XVARNA Study` | مقایسه گزینه‌ها، sensitivity و optimization hooks | P1 |
| `XVARNA Web` | viewer و اجرای مرورگری | P1؛ WebGPU compute می‌تواند Experimental باشد |

### 2.3 tagline

tagline اصلی پیشنهادی:

> **Design by Evidence.**

tagline توضیحی:

> **Real-time environmental and spatial intelligence for computational design.**

هیچ tagline نباید سرعت یا دقت اثبات‌نشده را ادعا کند.

### 2.4 نام‌گذاری فنی

- نام repository مادر: `xvarna`؛
- نام سازمان پیشنهادی: `xvarna-labs` یا `xvarna-engine`، مشروط به availability؛
- prefix کامپوننت‌های Grasshopper: `XV`؛
- namespace دات‌نت: `Xvarna.*`؛
- crate prefix: `xvarna-*`؛
- C ABI prefix: `xv_`؛
- schema namespace: `dev.xvarna.*`؛
- متغیرهای محیطی: `XVARNA_*`؛
- file extension اختصاصی cache در صورت نیاز: `.xvcache`؛
- file extension مطالعه قابل‌حمل در صورت نیاز: `.xvstudy`.

---

## 3. چشم‌انداز، مأموریت و اصول محصول

### 3.1 چشم‌انداز

XVARNA باید به زیرساخت مرجع متن‌باز برای پرس‌وجوی محیطی و فضایی سریع در طراحی محاسباتی تبدیل شود؛ زیرساختی که هم برای دانشجو قابل استفاده باشد، هم برای دفتر معماری قابل اتکا، و هم برای پژوهشگر قابل استناد و گسترش.

### 3.2 مأموریت نسخه 1.0

در اولین انتشار عمومی، کاربر باید بتواند از داخل Rhino/Grasshopper یک workflow کامل از مدل تا تصمیم و گزارش را اجرا کند، در حالی که:

- محاسبه interactive است؛
- نتیجه قابل توضیح است؛
- error و uncertainty پنهان نمی‌شوند؛
- backend و سخت‌افزار قابل انتخاب یا تشخیص خودکار است؛
- داده‌ها محلی باقی می‌مانند؛
- نتایج قابل بازتولید و export هستند؛
- benchmark و validation عمومی‌اند.

### 3.3 اصول غیرقابل مذاکره

1. **Evidence before marketing:** هیچ ادعای سرعت یا دقت بدون benchmark بازتولیدپذیر منتشر نمی‌شود.
2. **Correctness before cleverness:** نتیجه غلط سریع، شکست محصول است.
3. **Interactive by architecture:** performance یک feature جانبی نیست؛ از data layout تا API باید برای آن طراحی شوند.
4. **Explain the result:** هر metric اصلی باید امکان attribution یا توضیح محدودیت خود را داشته باشد.
5. **Local-first:** مدل معماری بدون اجازه کاربر به cloud ارسال نمی‌شود.
6. **Open core, open benchmark:** هسته اصلی، schemaها، test corpus و benchmark harness متن‌بازند.
7. **One model, many backends:** CPU/GPU/CLI/GH نباید مدل داده متفاوت و ناسازگار داشته باشند.
8. **Graceful degradation:** نبود GPU یا کمبود VRAM نباید محصول را بلااستفاده کند.
9. **No dependency maze:** نصب باید کنترل‌شده، نسخه‌بندی‌شده و قابل تشخیص باشد.
10. **Research-grade traceability:** نسخه engine، تنظیمات، seed، ورودی و hardware باید در گزارش ثبت شوند.
11. **Respect specialized engines:** در جایی که موتور مرجع دقیق‌تر است، XVARNA باید integrate و validate کند، نه اینکه ادعای غیرواقعی داشته باشد.
12. **Accessible defaults, expert control:** کاربر تازه‌کار preset خوب می‌گیرد؛ کاربر حرفه‌ای همه فرض‌ها را می‌بیند و تغییر می‌دهد.

---

## 4. مسئله و فرصت

### 4.1 مسائل اصلی کاربران

کاربران هدف با مجموعه‌ای از مشکلات تکرارشونده روبه‌رو هستند:

- کندی ray intersection روی مدل‌های پیچیده و gridهای متراکم؛
- محاسبه مجدد کل مدل پس از یک تغییر کوچک؛
- نبود attribution و ناتوانی در فهم علت heatmap؛
- پراکندگی workflow میان ابزارهای نور، دید، privacy و گزارش؛
- ناسازگاری pluginها و dependencyهای دشوار؛
- وابستگی بعضی راه‌حل‌های سریع به Windows یا NVIDIA؛
- تفاوت نتایج CPU/GPU و نبود گزارش uncertainty؛
- سختی تبدیل نتیجه تحلیل به objective برای optimization؛
- دشواری استفاده برای دانشجو و پیچیدگی بیش از حد برای iteration سریع؛
- نبود benchmark معماری عمومی و قابل تکرار؛
- قاطی‌شدن metricهای تقریبی early-stage با simulationهای validated.

### 4.2 فرصت محصول

فرصت XVARNA در ساختن «یک ray tracer دیگر» نیست. فرصت در ترکیب موارد زیر است:

- engine سریع و cache-aware؛
- backend چندسخت‌افزاری؛
- metricهای معماری آماده؛
- attribution در سطح object/time/ray؛
- UX یکپارچه Grasshopper؛
- fast mode و validated mode؛
- گزارش پژوهشی و audit trail؛
- API باز برای اکوسیستم.

### 4.3 Job To Be Done اصلی

> وقتی طرح پارامتریک را تغییر می‌دهم، می‌خواهم اثر آن بر نور، دید و حریم خصوصی را سریع و قابل توضیح ببینم تا بتوانم همان لحظه گزینه بهتر را انتخاب و بعداً از تصمیمم دفاع کنم.

---

## 5. کاربران هدف و personaها

### 5.1 دانشجوی معماری و طراحی محاسباتی

**نیازها:** نصب ساده، preset، نتیجه سریع، visualization خوب، خروجی مناسب ارائه و امکان فهم metric.  
**شکست فعلی:** زمان محدود، سخت‌افزار متوسط، سردرگمی بین ابزارها.  
**موفقیت:** اولین تحلیل معتبر در کمتر از 15 دقیقه پس از نصب، بدون نیاز به کدنویسی.

### 5.2 طراح پارامتریک حرفه‌ای

**نیازها:** Data Tree صحیح، cache، async execution، componentهای composable، API و خروجی optimization.  
**موفقیت:** تغییر پارامتر بدون freeze شدن Grasshopper و استفاده از نتیجه در مطالعه چندهدفه.

### 5.3 متخصص پایداری و daylight

**نیازها:** فرض‌های آشکار، مدل آسمان، metric استاندارد، validation، export و reproducibility.  
**موفقیت:** توانایی استفاده از XVARNA برای early-stage و ارسال مستقیم همان study به validation دقیق.

### 5.4 دفتر معماری و تیم مسابقه

**نیازها:** مدل بزرگ، چند گزینه، turnaround کوتاه، گزارش قابل ارائه و اشتراک نتایج.  
**موفقیت:** مقایسه ده‌ها گزینه در زمان قابل قبول و خروجی بصری قابل استفاده در جلسه تصمیم‌گیری.

### 5.5 پژوهشگر و استاد

**نیازها:** روش روشن، کد قابل بازرسی، dataset، benchmark، citation و API.  
**موفقیت:** بازتولید نتایج مقاله و امکان استفاده از هسته در پژوهش مستقل.

### 5.6 توسعه‌دهنده افزونه

**نیازها:** ABI پایدار، schema مستند، مثال binding، error model و test fixtures.  
**موفقیت:** ساخت یک connector یا metric جدید بدون fork کردن کل engine.

---

## 6. اهداف و non-goalها

### 6.1 اهداف نسخه 1.0

| ID | هدف | اولویت |
|---|---|---|
| `GOAL-001` | engine قابل استفاده روی CPU و حداقل یک backend GPU قابل‌حمل | P0 |
| `GOAL-002` | connector پایدار Rhino 8/GH1 و Rhino 9/GH1، به‌علاوه connector مستقل Rhino 9/GH2 مطابق SDK رسمی | P0 |
| `GOAL-003` | تحلیل کامل sun/shadow/sky/radiation اولیه | P0 |
| `GOAL-004` | تحلیل view/isovist/privacy/intervisibility | P0 |
| `GOAL-005` | attribution شیء و زمان برای metricهای پشتیبانی‌شده | P0 |
| `GOAL-006` | fast daylight به همراه workflow اعتبارسنجی Radiance | P1 |
| `GOAL-007` | cache و incremental scene update | P0 |
| `GOAL-008` | CLI و format قابل‌حمل study/result | P0 |
| `GOAL-009` | benchmark، validation report و test corpus عمومی | P0 |
| `GOAL-010` | viewer/report تعاملی و export پژوهشی | P1 |
| `GOAL-011` | hooks مناسب optimization و batch study | P1 |
| `GOAL-012` | مستندات کامل، exampleها و onboarding حرفه‌ای | P0 |

### 6.2 non-goalهای نسخه 1.0

موارد زیر آگاهانه خارج از محدوده 1.0 هستند، مگر اینکه RFC دامنه را تغییر دهد:

- ساخت CAD kernel یا B-rep modeler عمومی؛
- جایگزینی Rhino، Grasshopper یا Revit؛
- بازنویسی کامل Radiance؛
- شبیه‌سازی کامل HVAC و whole-building energy؛
- CFD عمومی یا جایگزینی OpenFOAM؛
- تحلیل سازه finite element؛
- مدل کامل انتقال حرارت پوسته؛
- cloud SaaS اجباری؛
- تولید فرم با LLM به‌عنوان قابلیت اصلی؛
- chatbot عمومی برای Grasshopper؛
- پشتیبانی رسمی هم‌زمان از همه نسخه‌های Rhino؛
- تضمین صحت برای هندسه نامحدود، مختصات نجومی یا داده خراب بدون هشدار؛
- ادعای compliance قانونی یا certification بدون فرآیند رسمی مستقل؛
- کنترل مستقیم و خطرناک sliderهای upstream در Grasshopper بدون قرارداد evaluator مشخص.

### 6.3 حوزه توسعه بعدی

نسخه‌های 1.x یا 2.x می‌توانند connectorهای Revit/Dynamo/Blender، backend ابری اختیاری، materialهای پیچیده، multi-bounce GPU، تحلیل صوت، wind proxy، thermal comfort و optimizer داخلی پیشرفته را اضافه کنند. این موارد نباید release gate نسخه 1.0 را مبهم کنند.

---

## 7. چشم‌انداز محصول و موضع‌گیری

### 7.1 محصولات مرجع و workflowهای هدف

محصولات موجود فقط برای شناخت نیاز کاربر، کشف شکاف قابلیت و تعیین baseline فنی مطالعه می‌شوند. مقایسه مستقیم یا ادعای جایگزینی کامل، شرط طراحی یا انتشار XVARNA نیست. حوزه‌های مرجع عبارت‌اند از:

- کامپوننت‌های CAD-ray در Rhino/Grasshopper؛
- workflowهای solar/view در Ladybug؛
- ابزارهای GPU real-time مانند Cyclops؛
- ابزارهای visibility/isovist مانند depthmapX؛
- اسکریپت‌های سفارشی MeshRay؛
- pipelineهای دستی Radiance برای early-stage؛
- افزونه‌های تک‌منظوره sun/shadow/view/privacy.

### 7.2 کیفیتی که باید ساخته شود

معیار موفقیت، حل سریع، دقیق و قابل‌بازتولید workflow هدف با یک scorecard قابل اندازه‌گیری است. قابلیت‌های بازار ورودی backlog هستند، نه مجوز استفاده از واژه‌هایی مانند «بهترین»، «سریع‌ترین» یا «بی‌رقیب»:

| محور | مزیت هدف XVARNA |
|---|---|
| سرعت interactive | cache، incremental update و GPU/CPU backend |
| پوشش سخت‌افزار | NVIDIA، AMD، Intel و CPU fallback |
| توضیح‌پذیری | attribution، top occluder و counterfactual |
| یکپارچگی | خورشید، دید، privacy، study و report در یک data model |
| بازبودن | کد، schema، benchmark و روش validation عمومی |
| reproducibility | seed، engine version، hardware و تنظیمات ثبت‌شده |
| UX | نصب ساده، preset و عدم freeze UI |
| پژوهش | test corpus، مقاله و citation |

### 7.3 سیاست مقایسه اخلاقی

- مستندات محصول بر قابلیت، روش، محدودیت و شواهد مطلق خود XVARNA تمرکز می‌کنند؛ مقایسه‌ی تبلیغاتی با رقبا انجام نمی‌شود.
- performance با زمان، throughput، حافظه، اندازه صحنه، سخت‌افزار، driver و تنظیمات بازتولیدپذیر گزارش می‌شود؛ صفت برتری جای عدد را نمی‌گیرد.
- وجود یک قابلیت در محصول دیگر برای requirement discovery استفاده می‌شود، اما روش XVARNA باید مستقل، قابل‌ممیزی و متناسب با معماری Rust/CPU/GPU آن طراحی شود.
- نام رقیب فقط همراه با نسخه، تاریخ، سخت‌افزار و تنظیمات ذکر می‌شود.
- benchmark cherry-pick نمی‌شود؛ sceneهای کوچک، متوسط، بزرگ و pathological منتشر می‌شوند.
- اگر ابزار رقیب برای metric خاص دقیق‌تر یا سریع‌تر است، نتیجه پنهان نمی‌شود.
- مقایسه با نرم‌افزار دارای محدودیت license فقط در چهارچوب مجاز انجام می‌شود.
- ادعاهای اشخاص ثالث به‌عنوان claim آن منبع نقل می‌شوند، نه حقیقت تأییدشده XVARNA.

---

## 8. معیارهای موفقیت و North Star

### 8.1 North Star Metric

معیار اصلی محصول:

> **Validated Design Iterations per Minute** — تعداد iteration طراحی که در یک دقیقه با نتیجه قابل استفاده و سطح دقت اعلام‌شده محاسبه می‌شوند.

این معیار باید هم زمان محاسبه، هم usability و هم validity را منعکس کند. نتیجه‌ای که سریع اما نامعتبر یا غیرقابل فهم باشد iteration موفق محسوب نمی‌شود.

### 8.2 KPIهای نسخه 1.0

| ID | KPI | هدف انتشار |
|---|---|---|
| `KPI-001` | crash-free analysis sessions | حداقل 99.5% در Closed Beta |
| `KPI-002` | موفقیت نصب روی سیستم‌های پشتیبانی‌شده | حداقل 95% بدون مداخله دستی |
| `KPI-003` | زمان اولین تحلیل نمونه | حداکثر 15 دقیقه برای کاربر جدید |
| `KPI-004` | پوشش تست هسته بحرانی | حداقل 85% line و 90% branch در ماژول‌های حساس، همراه mutation tests منتخب |
| `KPI-005` | تطابق visibility CPU/GPU | بیش از 99.9% queryها در tolerance تعریف‌شده |
| `KPI-006` | reproducibility | نتیجه deterministic برای config/seed/backend یکسان |
| `KPI-007` | سرعت نسبت به baseline CPU | هدف حداقل 5x در workloadهای منتخب؛ ادعا فقط پس از benchmark |
| `KPI-008` | update تعاملی دستگاه مرجع | P95 زیر 1 ثانیه برای workload مرجع پس از warm cache |
| `KPI-009` | issueهای P0 باز هنگام انتشار | صفر |
| `KPI-010` | documentation coverage کامپوننت‌ها | 100% همراه مثال حداقل برای هر گروه |
| `KPI-011` | beta validation | حداقل 20 کاربر و 10 پروژه واقعی موفق |
| `KPI-012` | benchmark reproducibility | اجرای one-command با artifact و metadata کامل |

اعداد performance پس از milestone baseline ممکن است با RFC و شواهد اصلاح شوند، اما نباید بدون ثبت تغییر کاهش یابند.

### 8.3 معیارهای اثر پژوهشی و عمومی

- release قابل cite با DOI؛
- preprint یا مقاله روش‌شناسی؛
- dataset و benchmark مستقل؛
- حداقل سه case study قابل بازتولید؛
- ویدئوی اصلی با داده واقعی، نه صحنه ساختگی بسیار کوچک؛
- استفاده بیرونی ثبت‌شده در پژوهش یا studio؛
- مشارکت حداقل چند توسعه‌دهنده بیرونی پس از انتشار؛
- roadmap و governance شفاف.

---

## 9. سناریوهای کاربری اصلی

### 9.1 تحلیل آفتاب سایت شهری

کاربر مدل سایت شامل ساختمان، زمین، درخت و سایبان را وارد می‌کند؛ EPW یا موقعیت جغرافیایی و دوره زمانی را تعیین می‌کند؛ sensor grid می‌سازد؛ Sun Hours و Radiation را اجرا می‌کند؛ نتیجه را رنگ‌آمیزی می‌کند؛ روی یک sensor ضعیف کلیک می‌کند و top occluderها و ساعات بحرانی را می‌بیند؛ سپس یک گزینه طراحی دیگر را مقایسه می‌کند.

### 9.2 طراحی سایبان

کاربر عمق و زاویه سایبان را پارامتریک می‌کند. XVARNA صحنه static ساختمان و dynamic سایبان را جدا نگه می‌دارد. با تغییر slider فقط BLAS/TLAS لازم update می‌شود. خروجی شامل کاهش تابش تابستان، حفظ آفتاب زمستان و attribution به اجزای سایبان است.

### 9.3 کیفیت دید واحدها

پنجره‌ها و نقاط دید به‌عنوان sensor، اهدافی مثل فضای سبز/آسمان/landmark به‌عنوان target و ساختمان‌های دیگر به‌عنوان occluder ثبت می‌شوند. محصول درصد solid angle قابل رؤیت، امتیاز وزنی، فاصله، جهت و مانع غالب را گزارش می‌کند.

### 9.4 حریم خصوصی نما

کاربر پنجره‌ها، balconies و مسیرهای عمومی را دسته‌بندی می‌کند. سامانه ماتریس intervisibility، pairهای بحرانی، زاویه دید، فاصله، exposure زمانی اختیاری و عناصر طراحی مسئول را نمایش می‌دهد.

### 9.5 annual daylight سریع و validated

کاربر fast mode را برای iteration اجرا می‌کند. هنگام رسیدن به گزینه نهایی، همان scene/material/sensor definition را به validated pipeline می‌فرستد. گزارش تفاوت، assumptions و metricهای sDA/ASE/UDI را ارائه می‌کند.

### 9.6 مطالعه چندگزینه‌ای

چند variant با شناسه و metadata وارد `XVARNA Study` می‌شوند. engine cache مشترک را استفاده می‌کند، metricها را محاسبه می‌کند، Pareto table می‌سازد و report HTML قابل اشتراک تولید می‌کند.

---

## 10. سطوح کیفیت محاسبه

هر تحلیل باید quality tier صریح داشته باشد. UI نباید نتایج tierهای متفاوت را بدون هشدار با هم مقایسه کند.

| Tier | نام | هدف | ویژگی |
|---|---|---|---|
| Q0 | Preview | feedback فوری | sample کم، approximation مجاز، watermark در report |
| Q1 | Interactive | طراحی روزمره | دقت کنترل‌شده، cache و GPU ترجیحی |
| Q2 | Standard | تصمیم طراحی | sample بیشتر، convergence check و uncertainty |
| Q3 | Research | پژوهش بازتولیدپذیر | تنظیمات کامل، seed ثابت، audit و validation |
| Q4 | Validated | مقایسه با engine مرجع | workflow Radiance/تحلیلی، report اختلاف |

### 10.1 قواعد tierها

- نتیجه Q0 نباید بدون label به‌عنوان نتیجه نهایی export شود.
- metricهایی که sampling دارند باید تعداد sample و uncertainty/convergence را گزارش کنند.
- backend می‌تواند میان tierها تغییر کند، اما semantics metric نباید تغییر پنهان داشته باشد.
- Q4 به معنی «کاملاً صحیح در همه شرایط» نیست؛ به معنی اجرا یا تطبیق با pipeline مرجع و ثبت روش است.
- preset هر tier نسخه‌بندی می‌شود؛ تغییر preset باید در changelog ذکر شود.

---

## 11. محدوده قابلیت‌های XVARNA 1.0

### 11.1 Core Scene and Geometry

| ID | نیازمندی | اولویت | معیار پذیرش خلاصه |
|---|---|---|---|
| `CORE-001` | ساخت scene از Rhino Mesh و mesh حاصل از Brep | P0 | topology، transform، object ID و units حفظ شوند |
| `CORE-002` | جداسازی لایه‌های static و dynamic | P0 | تغییر dynamic موجب rebuild بی‌دلیل static نشود |
| `CORE-003` | پشتیبانی از instance | P0 | یک geometry با transformهای متعدد بدون duplicate کامل حافظه |
| `CORE-004` | content hash و cache پایدار | P0 | ورودی یکسان cache hit قابل اثبات ایجاد کند |
| `CORE-005` | geometry validation | P0 | degenerate، NaN، index نامعتبر و scale خطرناک گزارش شوند |
| `CORE-006` | mesh repair محدود و غیرمخرب | P1 | remove degenerate، weld اختیاری و normal repair همراه report |
| `CORE-007` | origin rebasing | P0 | مدل‌های دور از origin با هشدار و دقت کنترل‌شده اجرا شوند |
| `CORE-008` | asynchronous jobs | P0 | UI Grasshopper هنگام اجرای طولانی responsive بماند |
| `CORE-009` | cancellation | P0 | job حداکثر در بودجه زمانی مشخص cancellation را مشاهده کند |
| `CORE-010` | progress reporting | P0 | phase، درصد تقریبی و زمان سپری‌شده گزارش شود |
| `CORE-011` | device selection و fallback | P0 | failure GPU به CPU fallback کنترل‌شده منجر شود |
| `CORE-012` | scene serialization | P1 | scene/study با schema نسخه‌بندی‌شده ذخیره و باز شود |
| `CORE-013` | metadata/category | P0 | هر object دارای ID، name، category و optional attributes باشد |
| `CORE-014` | clipping/region of interest | P1 | تحلیل روی subset فضایی بدون duplicate scene انجام شود |
| `CORE-015` | out-of-core chunking | P2 | صحنه بزرگ‌تر از VRAM به chunkهای قابل اجرا تقسیم شود |

**وضعیت 0.11.0:** `CORE-011` برای query batch عمومی و سه دامنه سنگین DAENA به‌صورت end-to-end تحویل شد: VAYU adapter discovery، انتخاب `Auto/GPU/CPU` و adapter-name filter یکسان، report تصمیم، precision/resource rejection و creation/runtime CPU fallback را از Rust تا C/.NET/CLI/Rhino 8/9 فراهم می‌کند. `RayQueryExecutor` تعریف علمی دامنه را از backend جدا کرده و Target/Weighted/Green View، Corridor و Observer Path را بدون fork الگوریتم روی CPU/GPU اجرا می‌کند. chunking فعلی مربوط به ray upload/dispatch/readback است؛ out-of-core geometry در `CORE-015` هنوز تحویل نشده است.

**وضعیت 0.12.0:** hot path پویا اکنون از دو scratch slot رشدپذیر و session-owned استفاده می‌کند؛ buffer، bind group و host staging پس از warm-up بدون allocation مجدد برای اندازه ثابت reuse می‌شوند و دو chunk در یک submission/poll window اجرا می‌شوند. callback رسمی device-loss و uncaptured-error، health state ماندگار، fallback سریع Auto و telemetry تجمعی از Rust تا C/.NET/Rhino/CLI تحویل شده‌اند. این تغییر `CORE-011` را production-grade‌تر می‌کند ولی out-of-core geometry در `CORE-015` همچنان مستقل و باز است.

**وضعیت 0.13.0:** `CORE-015` در سطح زیرساخت بسته شد: snapshot دولایهٔ TLAS/BLAS، instancing واقعی روی GPU، chunking قطعی تحت بودجهٔ resident geometry/VRAM، streaming از یک working set محدود، و parity کامل state/identity با CPU پیاده‌سازی شده‌اند. claim سرعت عمومی همچنان فقط پس از benchmark corpus چندسخت‌افزاری مجاز است.

### 11.2 ZURVAN Time and Weather

| ID | نیازمندی | اولویت | معیار پذیرش خلاصه |
|---|---|---|---|
| `TIME-001` | import فایل EPW | P0 | header، timezone، location و سری زمانی validate شوند |
| `TIME-002` | import WEA و CSV schema رسمی XVARNA | P1 | خطای ستون/واحد با شماره سطر گزارش شود |
| `TIME-003` | period و schedule | P0 | month/day/hour، timestep و wrap-year پشتیبانی شود |
| `TIME-004` | timezone و north rotation | P0 | true north/model north صریح و قابل تست باشند |
| `TIME-005` | sun position | P0 | با reference cases در tolerance توافق‌شده تطبیق کند |
| `TIME-006` | leap year و missing records | P0 | policy مشخص error/warn/interpolate داشته باشد |
| `TIME-007` | weather filters | P1 | occupancy، irradiance، temperature و custom mask |
| `TIME-008` | weighted timestep | P0 | هر ray/time دارای weight قابل ردیابی باشد |
| `TIME-009` | temporal attribution | P0 | ساعات/بازه‌های مؤثر برای sensor یا occluder استخراج شوند |
| `TIME-010` | calendar export | P1 | نتیجه زمانی به CSV/Parquet و chart report صادر شود |

### 11.3 HVARE Solar and ASMAN Sky

| ID | قابلیت | اولویت | خروجی اصلی |
|---|---|---|---|
| `SOL-001` | Sun Path و Sun Vectors | P0 | vector، altitude، azimuth، timestamp، weight |
| `SOL-002` | Direct Sun Hours | P0 | ساعت/درصد دریافت، binary timeline، attribution |
| `SOL-003` | Shadow Hours | P0 | ساعت سایه، دوره بحرانی، مانع غالب |
| `SOL-004` | Shadow Mask | P0 | ماسک hemispherical/azimuth-altitude |
| `SOL-005` | Solar Access | P0 | access ratio با threshold و schedule |
| `SOL-006` | Direct Irradiance | P0 | Wh/m² یا kWh/m² با DNI و incidence |
| `SOL-007` | Diffuse Sky Irradiance | P1 | contribution patchها با sky model انتخابی |
| `SOL-008` | Global Surface Irradiance | P1 | direct + diffuse + ground-reflected با assumptions |
| `SOL-009` | Sky View Factor | P0 | SVF و uncertainty/sample metadata |
| `SOL-010` | Solar Envelope | P1 | envelope با rule زمانی و tolerance مشخص |
| `SOL-011` | Shading Envelope | P1 | geometry محدودکننده برای حفاظت از دوره انتخابی |
| `SOL-012` | Facade/Roof Potential | P1 | ranking سطوح و breakdown زمانی |
| `SOL-013` | PV Potential Proxy | P1 | incident energy و model ساده losses؛ نه bankable yield |
| `SOL-014` | Ground Reflection | P2 | albedo-based contribution صریح |
| `SOL-015` | Multi-scenario Solar Compare | P1 | delta map و significance/uncertainty |

### 11.4 Daylight

دو مسیر محاسباتی تعریف می‌شود:

- **Fast Path:** engine داخلی برای feedback تعاملی و early-stage؛
- **Validated Path:** ساخت، اجرا و تفسیر jobهای Radiance با ثبت نسخه و تنظیمات.

| ID | قابلیت | مسیر | اولویت |
|---|---|---|---|
| `DL-001` | Point-in-Time Illuminance | Fast + Validated | P1 |
| `DL-002` | Daylight Factor | Fast + Validated | P1 |
| `DL-003` | sDA | Validated؛ Fast estimate برچسب‌دار | P1 |
| `DL-004` | ASE | Validated؛ direct fast check | P1 |
| `DL-005` | UDI | Validated؛ Fast estimate برچسب‌دار | P1 |
| `DL-006` | direct/diffuse breakdown | هر دو | P1 |
| `DL-007` | material reflectance/transmittance ساده | Fast | P1 |
| `DL-008` | Radiance material mapping | Validated | P1 |
| `DL-009` | glare proxy | Fast، بدون نام‌گذاری DGP | P2 |
| `DL-010` | image-based DGP workflow | Validated | P2 |
| `DL-011` | نتیجه اختلاف Fast/Validated | Report | P1 |
| `DL-012` | annual matrix reuse | هر دو | P1 |

قواعد:

- Fast estimate نباید به‌عنوان certification یا compliance معرفی شود.
- default metricها نسخه‌بندی و قابل تنظیم‌اند.
- در report باید model assumptions، sky model، material model و bounce count ثبت شوند.
- اگر Radiance نصب نیست، component validated باید راهنمای نصب/مسیر بدهد و fast mode را به‌اشتباه validated ننامد.

### 11.5 XVARNA Sight

| ID | قابلیت | اولویت | تعریف سطح بالا |
|---|---|---|---|
| `VIS-001` | Target View | P0 | سهم angular/solid-angle target قابل رؤیت |
| `VIS-002` | Weighted View Quality | P0 | ترکیب category، solid angle، distance و direction |
| `VIS-003` | Green View Index | P1 | سهم category سبز با تعریف viewpoint مشخص |
| `VIS-004` | Sky View | P0 | دید آسمان مستقل/هم‌خوان با SVF |
| `VIS-005` | 2D Isovist | P1 | polygon، area، perimeter، occlusivity و radial stats |
| `VIS-006` | 3D Isovist | P1 | sampled visibility volume/metrics |
| `VIS-007` | Intervisibility Matrix | P0 | sparse matrix میان دو مجموعه sensor/target |
| `VIS-008` | Privacy Risk | P0 | pair، distance، angle، exposure و weighting |
| `VIS-009` | View Corridor | P1 | فضای قابل حفاظت یا obstruction conflict |
| `VIS-010` | Landmark Visibility | P1 | visibility و prominence هدف |
| `VIS-011` | Street Canyon Visibility | P1 | sky/target metrics در مسیر خیابان |
| `VIS-012` | Visibility Graph | P1 | graph sparse، degree و centrality منتخب |
| `VIS-013` | Dynamic Observer Path | P1 | metric روی polyline/path با sample زمانی/فاصله‌ای |
| `VIS-014` | Occluder Map | P0 | first-hit object/triangle distribution |
| `VIS-015` | Category Attribution | P0 | سهم ساختمان/درخت/سایه‌بان و categoryهای کاربر |

**وضعیت 0.8.0:** موتور `DAENA` اکنون `VIS-005`، `VIS-007`، `VIS-008`، `VIS-012`، `VIS-014` و زیرساخت category-filtered برای `VIS-015` را به‌صورت end-to-end در Rust/C/.NET/CLI/Rhino 8/9 تحویل می‌دهد. Isovist به‌صراحت sampled approximation با finite range و convergence delta است؛ privacy یک screening proxy شفاف و نه probability/compliance claim است؛ visibility graph exact all-pairs با hard resource cap است.

**وضعیت 0.9.0:** `VIS-001`، `VIS-002`، `VIS-003`، `VIS-009` و `VIS-013` نیز end-to-end تحویل شدند. Target/Weighted/Green View از solid angle دقیق triangle و FOV، sampling قطعی Hammersley برای partial occlusion، weighting کاملاً آشکار، category masks، convergence و first-blocker attribution استفاده می‌کند. View Corridor با aperture دایره‌ای uniform-area و Dynamic Observer Path با tangent camera و trapezoidal distance weighting پیاده شده‌اند. نام واحد subsystem همان `DAENA` است؛ برای هر metric نام اسطوره‌ای جدا اضافه نمی‌شود تا API مبهم نشود.

### 11.6 Attribution and Explainability

| ID | نیازمندی | اولویت |
|---|---|---|
| `ATTR-001` | first-hit object ID برای queryهای مناسب | P0 |
| `ATTR-002` | first-hit triangle/instance ID در حالت diagnostic | P1 |
| `ATTR-003` | top-K occluder با count، weight و percentage | P0 |
| `ATTR-004` | temporal attribution | P0 |
| `ATTR-005` | category attribution | P0 |
| `ATTR-006` | highlight مستقیم مانع در Rhino | P0 |
| `ATTR-007` | counterfactual remove-one analysis | P1 |
| `ATTR-008` | delta attribution میان دو design option | P1 |
| `ATTR-009` | confidence/uncertainty label | P0 |
| `ATTR-010` | explanation JSON قابل export | P1 |

Counterfactual باید دقیقاً مشخص کند آیا واقعاً scene بدون object دوباره محاسبه شده یا از approximation استفاده شده است. approximation بدون label ممنوع است.

### 11.7 XVARNA Study and Optimization

| ID | قابلیت | اولویت |
|---|---|---|
| `STUDY-001` | ثبت variant با ID، پارامترها و metadata | P0 |
| `STUDY-002` | مقایسه side-by-side و delta map | P1 |
| `STUDY-003` | batch run با resume | P1 |
| `STUDY-004` | export objective scalar/vector | P0 |
| `STUDY-005` | Grasshopper-friendly Data Tree output | P0 |
| `STUDY-006` | Pareto table و filtering | P1 |
| `STUDY-007` | sensitivity از dataset variant | P1 |
| `STUDY-008` | uncertainty-aware ranking | P1 |
| `STUDY-009` | early rejection با bound معتبر | P2 |
| `STUDY-010` | checkpoint و provenance | P1 |

نسخه 1.0 باید با Galapagos/Wallacei و هر optimizerی که scalar/vector Grasshopper می‌پذیرد سازگار باشد. optimizer داخلی کامل release gate نیست. sensitivity فقط وقتی ارائه می‌شود که sampling design و تعداد نمونه برای روش انتخابی معتبر باشد؛ تولید نمودار گمراه‌کننده از داده ناکافی ممنوع است.

**وضعیت 0.9.0:** subsystem جدید `VAHMAN` (برگرفته از Vohu Manah / «اندیشه نیک») `STUDY-001`، `STUDY-004`، `STUDY-005`، `STUDY-006` و بخش descriptive از `STUDY-007` را تحویل می‌دهد. موتور شامل mixed continuous/integer/categorical variables، residual constraints با قرارداد `g(x) <= 0`، constraint-domination، parallel non-dominated sorting، NSGA-II crowding، exact 2D hypervolume، Spearman ρ توصیفی، ask/tell قطعی، stratified initialization، SBX، polynomial/discrete mutation و bounded historical Pareto archive است. Spearman در این نسخه causal/confidence-aware نیست و نباید با Sobol/Morris یا causal sensitivity اشتباه معرفی شود.

**وضعیت 0.16.0:** `STUDY-001` تا `STUDY-007` و `STUDY-010` در قرارداد پلتفرم persistent تحویل شده‌اند: manifest/schema نسخه‌دار، registry نام‌دار، scalar/vector objective واقعی، journaled batch/resume، checkpoint کامل PRNG/population/archive/pending، commit اتمیک و idempotent، constraint-aware Pareto، baseline comparison و delta map. بخش sensitivity اکنون Spearman association همراه bootstrap percentile CI، permutation p-value و Holm family-wise correction است؛ causal sensitivity، `STUDY-008` uncertainty-aware ranking و `STUDY-009` early rejection همچنان باز هستند.

### 11.8 Reporting and Export

| ID | نیازمندی | اولویت |
|---|---|---|
| `REP-001` | report HTML self-contained یا bundle کنترل‌شده | P0 |
| `REP-002` | export CSV | P0 |
| `REP-003` | export JSON schema-versioned | P0 |
| `REP-004` | export Parquet برای داده بزرگ | P1 |
| `REP-005` | export colored mesh/glTF | P1 |
| `REP-006` | ثبت engine/backend/hardware/config/seed | P0 |
| `REP-007` | legend، units و quality tier در هر visualization | P0 |
| `REP-008` | comparison report | P1 |
| `REP-009` | validation report | P0 |
| `REP-010` | citation و method summary خودکار | P1 |

---

## 12. کاتالوگ کامپوننت‌های Grasshopper

نام‌ها تا پیش از UX freeze قابل اصلاح‌اند، اما semantics و گروه‌بندی باید پایدار بماند.

### 12.1 گروه 00 — Setup

| نام کامپوننت | ورودی‌های کلیدی | خروجی‌های کلیدی |
|---|---|---|
| `XV Info` | refresh | engine/plugin version، devices، capability report |
| `XV Backend` | preference، device، memory budget | BackendConfig |
| `XV Devices` | refresh | adapter، driver، API، device class |
| `XV Runtime` | BackendConfig | trace/ray/dispatch، arena، transfer، fallback، device health |
| `XV Quality` | tier، overrides | QualityConfig |
| `XV Units` | model units، target units، tolerance | UnitConfig |
| `XV Diagnostics` | scene/job/result | warnings، timings، memory، log bundle |

### 12.2 گروه 01 — Scene

| نام کامپوننت | ورودی‌های کلیدی | خروجی‌های کلیدی |
|---|---|---|
| `XV Scene` | geometry، transforms، IDs، categories، mode | SceneHandle، report |
| `XV Scene Update` | SceneHandle، changed geometry/transforms | new handle، update stats |
| `XV Mesh Check` | geometry، repair policy | valid mesh، issue list، repair report |
| `XV Material` | type، reflectance، transmittance، roughness | MaterialDef |
| `XV Category` | geometry branch، name، attributes | TaggedGeometry |
| `XV Sensor Grid` | geometry، spacing/count، offset | points، normals، areas، SensorSet |
| `XV Sensor Set` | points، normals، weights، IDs | SensorSet |
| `XV Target Set` | geometry/points، category، weights | TargetSet |
| `XV Cache Control` | action، scope، budget | cache stats |

### 12.3 گروه 02 — Time and Weather

| نام کامپوننت | ورودی‌های کلیدی | خروجی‌های کلیدی |
|---|---|---|
| `XV Import EPW` | path | WeatherData، location، warnings |
| `XV Period` | start، end، timestep، weekdays | TimeMask |
| `XV Weather Filter` | WeatherData، expression/threshold | TimeMask، filtered data |
| `XV Sun Vectors` | location/weather، TimeMask، north | SunSet |
| `XV Sky Model` | weather، model، subdivision | SkyDef |

### 12.4 گروه 03 — Solar and Daylight

| نام کامپوننت | ورودی‌های کلیدی | خروجی‌های کلیدی |
|---|---|---|
| `XV Sun Hours` | Scene، SensorSet، SunSet، Quality | metric، timeline، attribution |
| `XV Shadow Mask` | Scene، SensorSet، resolution | mask، obstruction IDs |
| `XV Solar Access` | Scene، sensors، SunSet، rule | pass/fail، ratio، attribution |
| `XV Radiation` | Scene، sensors، weather/sky، materials | direct، diffuse، total، uncertainty |
| `XV Sky View` | Scene، sensors، sampling | SVF، hit distribution |
| `XV Solar Envelope` | site، context، SunSet، rule | mesh/brep approximation، report |
| `XV Shading Envelope` | protected sensors، SunSet، rule | envelope، conflicts |
| `XV PV Potential` | surfaces، weather، losses | incident energy، proxy yield |
| `XV Illuminance` | Scene، sensors، sky، method | lux، uncertainty، validation link |
| `XV Annual Daylight` | model، sensors، weather، metrics، method | sDA، ASE، UDI، report |
| `XV Radiance Validate` | study، radiance config | job، result، comparison |

### 12.5 گروه 04 — Sight and Privacy

| نام کامپوننت | ورودی‌های کلیدی | خروجی‌های کلیدی |
|---|---|---|
| `XV Target View` | Scene، observers، target meshes/IDs/categories/weights، FOV | raw/weighted/green solid-angle metrics، convergence، attribution |
| `XV Weighted View` | alias/future extractor روی Target View result | score، breakdown |
| `XV Isovist` | Scene، eyes، plane frame، rays/range | polygon، radial tree، metrics، occluder |
| `XV Isovist 3D` | observer، Scene، sampling/range | metrics، point cloud/mesh proxy |
| `XV Intervisibility` | Scene، observers، targets، facing/weights/rules | complete matrix، lines، privacy risk، attribution |
| `XV Privacy` | Intervisibility result، occupancy، thresholds | policy-specific risk، critical pairs، attribution |
| `XV Corridor` | origins، target apertures، radii، Scene | open fraction/solid angle، samples، conflict attribution |
| `XV Visibility Graph` | nodes، Scene، range/rules | graph، metrics |
| `XV View Path` | polyline، orientation، target meshes، spacing | distance-weighted time/space series، hotspots |
| `XV Occluders` | any supported result، top-K | IDs، shares، highlight data |

### 12.6 گروه 05 — Study and Results

| نام کامپوننت | ورودی‌های کلیدی | خروجی‌های کلیدی |
|---|---|---|
| `XV Study` | variant ID، parameters، results | StudyHandle |
| `XV Compare` | results/studies، baseline | delta، ranking، maps |
| `XV Objective` | results، aggregation، constraints | scalar/vector، validity |
| `XV Sensitivity` | study dataset، method | indices، confidence، warnings |
| `XV Pareto` | objective table، goals | frontier، ranks |
| `XV Colorize` | mesh/sensors، values، legend | display mesh، legend |
| `XV Select Result` | viewport selection، result | sensor details، occluders، timeline |
| `XV Report` | study/result، template | HTML bundle، assets |
| `XV Export` | data، format، path | artifact paths، hashes |
| `XV Benchmark` | scenario، repetitions، backends | benchmark record |
| `XV Optimizer` | variable kinds/bounds، objective directions، constraints، population/seed | native session، initial candidate tree |
| `XV Opt Step` | session، aligned objective/constraint trees، explicit pulse | Pareto/archive state، next candidate tree |

### 12.7 قرارداد عمومی کامپوننت‌ها

تمام کامپوننت‌های محاسباتی MUST:

- input validation قبل از شروع job داشته باشند؛
- از اجرای ناخواسته سنگین هنگام تغییر جزئی UI جلوگیری کنند؛
- cancellation را بپذیرند؛
- progress و phase را گزارش کنند؛
- warning را از error جدا کنند؛
- unit و quality tier را در tooltip/output metadata ثبت کنند؛
- از Data Tree semantics مستند استفاده کنند؛
- output قبلی را هنگام job جدید با label `stale` نگه دارند یا طبق policy پاک کنند؛
- هیچ exception native را بدون ترجمه به UI عبور ندهند؛
- امکان bake نتیجه همراه metadata را فراهم کنند، اگر geometry output دارند.

---

## 13. پلتفرم‌ها و سازگاری

### 13.1 پلتفرم‌های هدف

| پلتفرم | سطح پشتیبانی 1.0 |
|---|---|
| Windows 11 x64 + Rhino 8 | Tier A، release blocker |
| Windows 11 x64 + Rhino 9 | Tier A، release blocker؛ تا زمان انتشار Rhino 9 با SDK/WIP رسمی تست می‌شود |
| Windows 10 x64 در صورت پشتیبانی Rhino/driver | Tier B، best effort با CI محدود |
| macOS Apple Silicon + Rhino 8/9 | Tier B؛ باید قبل از 1.0 روی دستگاه واقعی validate شود |
| macOS Intel | P2/best effort |
| Linux x64 CLI | Tier B |
| Browser WebGPU | P1 برای viewer؛ compute می‌تواند Experimental باشد |
| Revit/Dynamo | خارج از 1.0 |
| Rhino.Compute/Server | 1.x، مگر نیاز پژوهشی زودتر ایجاد شود |

### 13.2 سخت‌افزار

- CPU-only باید کاملاً قابل استفاده باشد.
- GPU backend باید حداقل یک مسیر portable داشته باشد.
- NVIDIA-specific acceleration قابلیت افزوده است، نه dependency اجباری.
- Intel iGPU باید برای workload کوچک/متوسط تست شود.
- AMD باید پیش از ادعای رسمی روی حداقل یک دستگاه واقعی تست شود.
- حداقل RAM هدف: 16 GB؛ 32 GB برای scene بزرگ توصیه می‌شود.
- حداقل VRAM ثابت اعلام نمی‌شود؛ capability detection و memory budget تعیین‌کننده‌اند.

### 13.2.1 ماتریس runtime و SDK Rhino

XVARNA connector به‌صورت multi-target و `AnyCPU` ساخته می‌شود:

| محصول | Target Framework | SDK reference | سیاست |
|---|---|---|---|
| Rhino 8.20+ | `net8.0` | Grasshopper/RhinoCommon 8.x pin‌شده | حداقل رسمی نسل 8 برای XVARNA |
| Rhino 9 + Grasshopper 1 | `net10.0` | Grasshopper/RhinoCommon 9.x pin‌شده | build و package مستقل Rhino 9 |
| Rhino 9 + Grasshopper 2 | `net10.0` | `Grasshopper2 9.0.26237.15343-beta` با pin دقیق | connector مستقل 49 قابلیتی؛ runtime/semantic/numerical gate سبز، interactive canvas acceptance باقی است |

- `net7.0` برای کد جدید XVARNA هدف نمی‌شود؛ runtime اولیه Rhino 8 بوده اما نسل جاری Rhino 8 روی .NET 8 است.
- `net48` برای محصول standalone جدید هدف نمی‌شود؛ مسیر .NET Framework در Rhino 9 deprecated است.
- خروجی‌های Rhino 8 و Rhino 9 در package/manifest جدا می‌شوند تا SDK نسل‌ها مخلوط نشود.
- source مشترک می‌ماند و کد شرطی فقط برای API break واقعی مجاز است.
- CI هر دو target را build می‌کند؛ integration test واقعی Rhino 9 تا زمان نصب WIP/نسخه رسمی در runner جدا انجام می‌شود.
- native core از connector مستقل است و ABI یکسان دارد؛ فقط binary مطابق OS/architecture بسته‌بندی می‌شود.
- GH2 بازکامپایل ساده‌ی assembly مربوط به GH1 نیست. پروژه‌ی `Xvarna.Grasshopper2` فقط لایه‌ی component/data-tree/preview/scheduler میزبان را پیاده می‌کند و تمام محاسبه، provenance و schema را از `Xvarna.Native` و Rust core مشترک می‌گیرد.
- در وضعیت فعلی SDK، `Grasshopper2` به RhinoCommon 9 prerelease وابسته است؛ بنابراین Rhino 8 + GH2 جزو compatibility claim نیست. این تصمیم با تغییر قرارداد رسمی McNeel دوباره بررسی می‌شود.

### 13.3 دستگاه توسعه مرجع فعلی

| مورد | مقدار ثبت‌شده در تاریخ سند |
|---|---|
| OS | Windows 11 Home 64-bit، build family 10.0.26200 |
| CPU | Intel Core i7-14650HX، 16 core / 24 logical processor |
| RAM | حدود 16 GB |
| GPU 1 | NVIDIA GeForce RTX 5060 Laptop GPU، حافظه گزارش‌شده حدود 4 GB |
| GPU 2 | Intel UHD Graphics |
| Rhino | 8.27.25357.11371 |
| Rust | rustc/cargo 1.96.0، target پیش‌فرض `x86_64-pc-windows-msvc` |
| .NET SDK | 10.0.300 |
| Python | 3.14.2؛ فقط ابزار جانبی/validation |

این دستگاه تنها reference اصلی performance نیست. benchmark نهایی باید حداقل یک CPU-only، یک Intel/AMD و یک NVIDIA را پوشش دهد.

### 13.4 ابزارهای لازم پیش از build

محیط فعلی باید با موارد زیر تکمیل شود:

- Visual Studio Build Tools با Desktop C++ workload و Windows SDK؛
- linker سازگار MSVC؛
- CMake و Ninja برای dependencyهای native احتمالی؛
- Git؛
- target/SDK دقیق مورد نیاز template رسمی Rhino 8؛
- ابزار packaging Rhino/Yak؛
- در صورت backend تخصصی: CUDA Toolkit/OptiX SDK با نسخه pin‌شده؛
- Radiance برای validation؛ ترجیحاً در محیط isolate و نسخه ثبت‌شده.

نسخه دقیق dependencyها در lockfile/toolchain file و build documentation pin می‌شود.

---

## 14. معماری کلان سیستم

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Clients                                                             │
│  Grasshopper Plugin │ Rhino Commands │ CLI │ Python │ Web Viewer    │
└───────────────┬─────────────────────────────────────────────────────┘
                │ typed .NET API / C ABI / serialized schema
┌───────────────▼─────────────────────────────────────────────────────┐
│ xvarna-api / xvarna-ffi                                             │
│ versioning │ validation │ async jobs │ errors │ memory ownership    │
└───────────────┬─────────────────────────────────────────────────────┘
                │
┌───────────────▼─────────────────────────────────────────────────────┐
│ Domain Services                                                     │
│ HVARE │ ZURVAN │ SIGHT │ ATTRIBUTION │ STUDY │ RASHNU              │
└───────────────┬─────────────────────────────────────────────────────┘
                │ scene/query contracts
┌───────────────▼─────────────────────────────────────────────────────┐
│ XVARNA Core                                                         │
│ geometry ingest │ BLAS/TLAS │ cache │ scheduler │ result store      │
└───────┬─────────────────────┬─────────────────────┬─────────────────┘
        │                     │                     │
┌───────▼────────┐    ┌───────▼──────────┐  ┌──────▼────────────────┐
│ CPU Backend    │    │ Portable GPU     │  │ Specialized Backends │
│ Rayon + SIMD   │    │ wgpu + WGSL      │  │ OptiX/Radiance bridge│
└────────────────┘    └──────────────────┘  └───────────────────────┘
```

### 14.1 قواعد معماری

- domain metricها نباید مستقیماً به RhinoCommon وابسته باشند.
- هیچ type مربوط به Rust ownership نباید از C ABI عبور کند.
- geometry source و compute scene از هم جدا هستند.
- backend باید interface مشترک query داشته باشد.
- نتیجه باید مستقل از UI قابل serialize باشد.
- plugin نباید DLLهای متعدد ناسازگار را در مسیر عمومی Rhino رها کند.
- dependencyهای native باید private و نسخه‌بندی‌شده در package باشند.
- failure یک backend نباید process Rhino را crash کند؛ تا حد امکان isolation و error boundary لازم است.
- API داخلی می‌تواند سریع تغییر کند؛ API عمومی پس از freeze فقط با semantic versioning.

### 14.2 لایه‌ها

1. **Adapters:** Rhino/GH، CLI، Web، Python؛
2. **Application:** job orchestration، config، validation و reporting؛
3. **Domain:** metricها و semantics معماری؛
4. **Core:** scene، acceleration structure، ray/visibility query؛
5. **Backends:** CPU/GPU/reference engine؛
6. **Infrastructure:** cache، serialization، logging، packaging.

وابستگی باید رو به پایین باشد. Domain نباید adapter را import کند.

---

## 15. ساختار پیشنهادی repository

```text
xvarna/
├─ Cargo.toml
├─ rust-toolchain.toml
├─ global.json
├─ LICENSE
├─ NOTICE
├─ CITATION.cff
├─ CODE_OF_CONDUCT.md
├─ CONTRIBUTING.md
├─ SECURITY.md
├─ GOVERNANCE.md
├─ CHANGELOG.md
├─ README.md
├─ crates/
│  ├─ xvarna-types/
│  ├─ xvarna-geometry/
│  ├─ xvarna-scene/
│  ├─ xvarna-daena/
│  ├─ xvarna-bvh/
│  ├─ xvarna-query/
│  ├─ xvarna-cpu/
│  ├─ xvarna-gpu/
│  ├─ xvarna-hvare/
│  ├─ xvarna-zurvan/
│  ├─ xvarna-attribution/
│  ├─ xvarna-study/
│  ├─ xvarna-rashnu/
│  ├─ xvarna-io/
│  ├─ xvarna-report/
│  ├─ xvarna-ffi/
│  └─ xvarna-cli/
├─ dotnet/
│  ├─ Xvarna.Native/
│  ├─ Xvarna.Grasshopper/
│  ├─ Xvarna.Rhino/
│  └─ Xvarna.Tests/
├─ shaders/
│  ├─ common/
│  ├─ build/
│  └─ traversal/
├─ web/
│  ├─ app/
│  └─ wasm/
├─ schemas/
│  ├─ scene/
│  ├─ study/
│  └─ result/
├─ benchmarks/
│  ├─ harness/
│  ├─ manifests/
│  ├─ scenes/
│  └─ baselines/
├─ validation/
│  ├─ analytic/
│  ├─ radiance/
│  └─ reports/
├─ examples/
│  ├─ beginner/
│  ├─ architecture/
│  ├─ urban/
│  └─ research/
├─ docs/
│  ├─ architecture/
│  ├─ algorithms/
│  ├─ components/
│  ├─ tutorials/
│  └─ research/
├─ packaging/
│  ├─ yak/
│  ├─ windows/
│  ├─ macos/
│  └─ radiance/
└─ tools/
   ├─ xtask/
   ├─ dataset/
   └─ release/
```

### 15.1 قواعد repository

- `main` همیشه buildable است.
- featureها با branch کوتاه یا PR توسعه می‌یابند.
- binary، dataset بزرگ و artifact release در Git معمولی commit نمی‌شوند؛ از release asset/LFS با policy روشن استفاده می‌شود.
- benchmark scene باید license و provenance داشته باشد.
- هر crate public API باید rustdoc داشته باشد.
- هر component GH باید help، screenshot و example داشته باشد.
- shaderها تست host-side و در صورت امکان differential test دارند.
- code generation فقط از script versioned و reproducible انجام می‌شود.

---

## 16. مدل داده هنجاری

### 16.1 انواع شناسه

| Type | اندازه منطقی | معنا |
|---|---|---|
| `SceneId` | 128-bit UUID یا hash namespaced | هویت منطقی scene |
| `SceneRevision` | `u64` | نسخه افزایشی immutable snapshot |
| `ObjectId` | `u64` | هویت پایدار شیء منبع |
| `InstanceId` | `u64` | occurrence یک geometry |
| `MeshId` | `u64` | geometry resource قابل اشتراک |
| `TriangleId` | `u32` در هر mesh chunk | primitive محلی |
| `SensorId` | `u64` | sensor پایدار |
| `TargetId` | `u64` | هدف دید/تحلیل |
| `MaterialId` | `u32` | material table index |
| `CategoryId` | `u32` | category table index |
| `JobId` | UUID | job async |
| `StudyId`/`VariantId` | UUID/string validated | مطالعه و گزینه |

IDها نباید از index موقت Data Tree ساخته شوند مگر آنکه policy صریح باشد. mapping میان Rhino object GUID و `ObjectId` باید پایدار، قابل serialize و قابل گزارش باشد.

### 16.2 Scene Snapshot

`SceneSnapshot` یک ساختار immutable است که حداقل شامل موارد زیر است:

- `scene_id`؛
- `revision`؛
- `schema_version`؛
- coordinate/unit metadata؛
- geometry resources؛
- instances و transforms؛
- object/category/material tables؛
- static/dynamic classification؛
- bounds و origin rebase؛
- hashes؛
- backend-independent acceleration metadata؛
- warnings و ingest report.

هر update، snapshot جدید می‌سازد یا handle جدیدی به revision تازه برمی‌گرداند. mutation پنهان که نتیجه job هم‌زمان را تغییر دهد ممنوع است.

### 16.3 هندسه

مدل canonical ورودی:

```text
MeshResource
  positions: [f64; 3] source/canonical
  indices:   [u32; 3]
  normals:   optional [f32; 3]
  material_per_face: optional u32
  local_bounds: AABB<f64>
  content_hash: Hash256
```

مدل compute می‌تواند برای GPU از `f32` استفاده کند، اما فقط پس از:

1. تبدیل واحد؛
2. origin rebasing؛
3. بررسی dynamic range؛
4. ثبت quantization/error bound؛
5. fallback به CPU/f64 یا chunking اگر precision budget نقض شود.

### 16.4 Transform و instance

- transform canonical ماتریس affine 4×4 با `f64` است.
- non-uniform scale پشتیبانی می‌شود اما normal transform باید inverse-transpose باشد.
- singular transform error است.
- mirrored transform flag می‌گیرد تا winding و normal semantics مشخص باشد.
- motion blur یا transform پیوسته خارج از 1.0 است؛ مسیر observer و timestep با snapshotهای گسسته مدل می‌شود.

### 16.5 SensorSet

هر sensor حداقل می‌تواند شامل موارد زیر باشد:

```text
Sensor
  id: u64
  position: f64x3
  normal: f32x3 | none
  area: f64 | none
  weight: f64 = 1
  group: u32 | none
  metadata: compact key/value references
```

- normal برای metricهای directional الزامی است.
- normal باید normalize شود؛ zero vector error است.
- area برای energy integration لازم است؛ نبود آن باید مانع گزارش total energy شود.
- sensor offset باید در metadata ثبت شود.

### 16.6 Ray Batch

مدل منطقی:

```text
RayBatch
  origins
  directions
  t_min
  t_max
  weights
  sensor_ids
  sample_ids
  time_ids (optional)
  query_flags
```

پیاده‌سازی backend می‌تواند AoS یا SoA باشد. برای hot path، SoA یا AoSoA و alignment مناسب ترجیح دارد. `direction` باید finite و normalized یا همراه scale semantics صریح باشد.

### 16.7 Hit Record

```text
Hit
  hit: bool
  distance: f32/f64
  object_id
  instance_id
  mesh_id
  triangle_id
  barycentric_uv (diagnostic/optional)
  front_face
  material_id
  flags
```

queryهای Boolean می‌توانند payload حداقلی برگردانند. attribution به `ObjectId` و `InstanceId` نیاز دارد. triangle و barycentric فقط وقتی فعال شوند که هزینه پذیرفته شده باشد.

### 16.8 Result Envelope

هر نتیجه مستقل از metric باید header مشترک داشته باشد:

- result UUID؛
- metric ID و semantic version؛
- scene ID/revision/hash؛
- sensor set hash؛
- time/weather hash؛
- quality tier و config hash؛
- backend/device/driver؛
- engine/plugin versions؛
- start/end/duration؛
- seed و sampling method؛
- units؛
- warnings؛
- validity state؛
- uncertainty/convergence summary؛
- provenance chain.

### 16.9 Validity State

هر result یکی از وضعیت‌های زیر را دارد:

- `Valid`؛
- `ValidWithWarnings`؛
- `Approximate`؛
- `Stale`؛
- `Cancelled`؛
- `Partial`؛
- `Invalid`؛
- `ReferenceValidated`.

UI و export نباید این وضعیت را حذف کنند.

---

## 17. واحدها، مختصات و دقت عددی

### 17.1 واحد canonical

- واحد داخلی domain برای فاصله **meter** است.
- ورودی Rhino با scale صریح به meter تبدیل می‌شود.
- خروجی UI می‌تواند در model units یا SI نمایش داده شود، اما metadata همیشه واحد canonical را نگه می‌دارد.
- انرژی سطحی با `Wh/m²` یا `kWh/m²` و illuminance با `lux` گزارش می‌شود.
- زاویه در API داخلی radian و در UI قابل نمایش به degree است.
- زمان canonical timestamp محلی همراه timezone و UTC offset ثبت‌شده است.

### 17.2 دستگاه مختصات

- coordinate system باید right-handed بودن، up-axis و north rotation را صریح ثبت کند.
- Rhino adapter مسئول تبدیل به قرارداد core است.
- true north و model north دو مقدار مستقل‌اند.
- location جغرافیایی به هندسه world coordinate تزریق نمی‌شود؛ در metadata باقی می‌ماند.

### 17.3 tolerance policy

یک epsilon ثابت جهانی ممنوع است. tolerance از ترکیب زیر محاسبه می‌شود:

- model absolute tolerance؛
- scene diagonal؛
- sensor offset؛
- backend precision؛
- ray distance range؛
- metric semantics.

پارامترهای اصلی:

- `geom_abs_tol_m`؛
- `geom_rel_tol`؛
- `ray_origin_offset_m`؛
- `parallel_epsilon`؛
- `barycentric_epsilon`؛
- `max_coordinate_magnitude`؛
- `gpu_quantization_bound_m`.

هر override باید در result ثبت شود. UI باید preset امن ارائه کند و override خطرناک را هشدار دهد.

### 17.4 self-intersection

rayهای خارج‌شونده از surface با policy زیر ساخته می‌شوند:

1. position canonical؛
2. normal validate و orient؛
3. offset scale-aware در جهت مناسب؛
4. `t_min` مستقل از offset؛
5. تست backface policy؛
6. ثبت مقدار offset در metadata.

استفاده از offset بزرگ که سایه نزدیک را حذف کند ممنوع است. validation corpus باید شامل سطوح بسیار نزدیک و coplanar باشد.

### 17.5 مختصات بزرگ

برای مدل‌های GIS یا دور از origin:

- scene origin نزدیک مرکز working bounds انتخاب می‌شود؛
- positionها قبل از GPU cast rebase می‌شوند؛
- transform بازگشت به world حفظ می‌شود؛
- precision estimator قبل از اجرا error bound را محاسبه می‌کند؛
- اگر budget نقض شد، backend باید warn/fallback/chunk کند، نه اینکه بی‌صدا نتیجه خراب بدهد.

---

## 18. ingest و آماده‌سازی هندسه

### 18.1 pipeline

```text
Source Geometry
  → adapter extraction
  → unit/coordinate normalization
  → finite/index validation
  → degeneracy analysis
  → optional repair
  → triangulation policy
  → normal/material/category mapping
  → content hash
  → mesh resource deduplication
  → BLAS build/load
  → instance/TLAS build
```

### 18.2 triangulation

- core فقط triangle primitive را در hot path 1.0 تضمین می‌کند.
- Brep/NURBS در adapter با setting نسخه‌بندی‌شده mesh می‌شوند.
- meshing parameters در scene provenance ثبت می‌شوند.
- تغییر meshing setting باید hash را تغییر دهد.
- quad به دو triangle باید deterministic باشد.
- polygon concave در importer CLI با triangulator تست‌شده پردازش می‌شود.

### 18.3 validation هندسه

issueهای قابل تشخیص:

- vertex غیر finite؛
- index خارج محدوده؛
- triangle با area نزدیک صفر؛
- duplicate face؛
- inconsistent winding؛
- non-manifold edge؛
- naked edge؛
- self-intersection احتمالی؛
- normal گمشده/صفر؛
- transform singular؛
- مختصات خارج precision budget؛
- category/material گمشده؛
- geometry empty.

همه issueها برای ray visibility مانع نیستند. validator باید severity را بر اساس metric تعیین کند. برای مثال mesh باز می‌تواند occluder معتبر باشد، ولی برای برخی volume metricها نامعتبر است.

### 18.4 repair policy

repair به‌صورت default محافظه‌کار است:

- حذف triangle degenerate؛
- حذف vertex بدون استفاده؛
- weld فقط با tolerance صریح؛
- recompute normal اختیاری؛
- orient connected components اختیاری؛
- hole filling عمومی در P0 نیست؛
- هر تغییر همراه before/after count و hash ثبت می‌شود؛
- geometry منبع Rhino بی‌اجازه mutate نمی‌شود.

### 18.5 category semantics

categoryهای built-in فقط convenience هستند:

- `ContextBuilding`؛
- `DesignBuilding`؛
- `Terrain`؛
- `VegetationOpaque`؛
- `VegetationPorous`؛
- `Shade`؛
- `Glazing`؛
- `TargetGreen`؛
- `TargetWater`؛
- `TargetSky`؛
- `PublicPath`؛
- `PrivateOpening`؛
- `Ignored`.

کاربر می‌تواند category سفارشی بسازد. semantics فیزیکی نباید فقط از نام category حدس زده شود؛ material/query policy باید صریح باشد.

---

## 19. acceleration structure و update strategy

### 19.1 مدل BLAS/TLAS

- هر `MeshResource` یک BLAS دارد.
- instanceها در TLAS قرار می‌گیرند.
- geometry مشترک میان instanceها BLAS مشترک دارد.
- static و dynamic می‌توانند TLAS یا subtree مجزا داشته باشند.
- query API می‌تواند layer mask بپذیرد.

### 19.2 BVH build

نسخه اول باید حداقل این مسیرها را ارزیابی کند:

- binned SAH برای کیفیت traversal؛
- LBVH/HLBVH برای build سریع dynamic؛
- hybrid build برای sceneهای بزرگ؛
- backend native acceleration structure در صورت دسترسی پایدار.

انتخاب نهایی با benchmark روی workload معماری انجام می‌شود. معیار فقط build time نیست؛ مجموع `build + N queries + update frequency + memory` معیار تصمیم است.

### 19.3 node layout

CPU و GPU ممکن است layout متفاوت داشته باشند، اما serializer/cache باید versioned باشد. گزینه‌های قابل benchmark:

- binary BVH؛
- BVH4/BVH8 برای SIMD؛
- quantized bounds؛
- depth-first flattened nodes؛
- rope/stackless traversal؛
- short stack traversal.

هیچ layout قبل از داده benchmark به‌عنوان برتر فرض نمی‌شود.

### 19.4 update classification

هر تغییر به یکی از این دسته‌ها طبقه‌بندی می‌شود:

1. metadata-only؛
2. material-only؛
3. rigid transform instance؛
4. vertex deformation با topology ثابت؛
5. topology change؛
6. add/remove instance؛
7. add/remove mesh resource؛
8. global unit/coordinate change.

policy:

- metadata/material: acceleration بدون rebuild؛
- transform: TLAS refit/rebuild؛
- deformation: BLAS refit در صورت quality مناسب، وگرنه rebuild؛
- topology: BLAS rebuild؛
- global scale/coordinate: full rebuild؛
- refit degradation با surface area ratio/quality heuristic پایش می‌شود.

### 19.5 cache keys

cache key باید حداقل شامل موارد زیر باشد:

- geometry content hash؛
- transform hash در سطح مربوط؛
- build algorithm/version؛
- precision/layout؛
- backend/device compatibility؛
- engine major/minor schema؛
- meshing/tolerance settings.

driver version فقط وقتی در key می‌آید که artifact به driver وابسته باشد.

### 19.6 traversal query types

- `AnyHit` برای visibility boolean؛
- `ClosestHit` برای distance/attribution؛
- `MultiHit` محدود و opt-in؛
- `CountHits` برای diagnostic/porous model؛
- `TargetFilteredHit` با layer/category mask؛
- `SegmentVisibility` برای intervisibility؛
- `OcclusionBatch` برای sun/sky؛
- `NearestDistance` در صورت نیاز spatial diagnostic، P2.

---

## 20. الگوریتم‌های پایه query

### 20.1 ray-triangle intersection

پیاده‌سازی باید watertight یا نزدیک به watertight برای edge-sharing triangles باشد و در برابر موارد زیر test شود:

- ray دقیقاً روی edge/vertex؛
- triangle بسیار باریک؛
- مختصات بزرگ پس از rebase؛
- جهت‌های نزدیک parallel؛
- front/back face؛
- transform mirrored؛
- ray بسیار کوتاه و بسیار بلند؛
- CPU f64 reference در برابر CPU/GPU f32.

reference implementation دقیق‌تر برای differential testing نگه داشته می‌شود، حتی اگر برای production کند باشد.

### 20.2 hemisphere sampling

روش‌های پشتیبانی‌شده:

- Fibonacci hemisphere؛
- stratified equal-area؛
- Sobol/low-discrepancy در صورت پیاده‌سازی معتبر؛
- sky-patch directions برای مدل‌های استاندارد؛
- custom direction set.

sampling باید deterministic با seed مشخص باشد. برای sensor normal، basis پایدار و تست‌شده ساخته می‌شود. sample weightها باید مجموع solid angle مناسب داشته باشند.

### 20.3 visibility semantics

visibility باید بین این حالات تفاوت بگذارد:

- binary unoccluded؛
- transmittance-weighted؛
- first opaque hit؛
- target-before-occluder؛
- range-limited؛
- FOV-limited؛
- backface culling on/off؛
- two-sided material؛
- category mask.

default هر component در help و result config ثبت می‌شود.

### 20.4 porous vegetation

نسخه 1.0 می‌تواند دو مدل ارائه کند:

1. opaque geometry؛
2. stochastic/aggregate transmittance ساده و برچسب‌دار.

مدل porous دقیق canopy خارج از P0 است. اگر stochastic استفاده شد، seed و احتمال عبور ثبت و uncertainty گزارش می‌شود.

---

## 21. تعریف metricها

### 21.1 Direct Sun Hours

برای sensor `i` و timestep `t`:

```text
visible(i,t) = 1 اگر پرتو به سمت خورشید در محدوده معتبر بدون مانع باشد
sun_hours(i) = Σ_t visible(i,t) × duration_hours(t) × schedule_weight(t)
```

قواعد:

- خورشید زیر حد altitude تنظیم‌شده حذف می‌شود؛
- incidence پشت سطح، بر اساس normal policy، دریافت محسوب نمی‌شود؛
- duplicate timestamp خطا یا deduplicate policy می‌گیرد؛
- attribution برای timestep مسدودشده از first hit محاسبه می‌شود؛
- نتیجه هم ساعت و هم ratio قابل ارائه دارد.

### 21.2 Solar Irradiance

direct contribution سطح:

```text
E_direct = Σ_t DNI(t) × max(0, n·s_t) × visible(t) × Δt
```

diffuse contribution بر اساس sky patch radiance/irradiance و visibility وزن‌دار محاسبه می‌شود. ground-reflected contribution فقط با albedo و assumption صریح فعال می‌شود. weather missing/negative values policy مشخص دارند.

### 21.3 Sky View Factor

SVF باید نسبت integral بخش قابل رؤیت hemisphere با weighting انتخابی باشد. محصول باید میان این تعریف‌ها تفکیک کند:

- unweighted visible hemisphere fraction؛
- cosine-weighted SVF؛
- patch-model SVF.

نام خروجی و report نباید این تعاریف را مخلوط کند.

### 21.4 Target View

Target View می‌تواند بر اساس یکی از denominatorهای زیر تعریف شود:

- کل FOV sample شده؛
- کل solid angle hemisphere؛
- فقط rayهایی که به target universe می‌رسند؛
- projected screen area در camera مشخص.

default باید solid-angle/FOV-based باشد. distance و category weight جدا از visibility خام گزارش می‌شوند تا score قابل تفسیر بماند.

### 21.5 Weighted View Quality

فرم عمومی:

```text
score_i = normalize(Σ_r visible_target(r) × solid_angle_weight(r)
                    × category_weight(r) × distance_weight(r)
                    × direction_weight(r))
```

تابع weightها و normalization باید در config/result قابل مشاهده باشد. presetها نباید به‌عنوان حقیقت جهانی معرفی شوند.

### 21.6 Privacy Risk

privacy یک metric قراردادی است، نه قانون جهانی. مدل پایه می‌تواند ترکیب زیر باشد:

- line-of-sight؛
- فاصله؛
- زاویه نسبت به normal بازشو/دید؛
- overlap FOV؛
- اندازه angular target؛
- occupancy/time weight؛
- public/private category؛
- وجود مانع نیمه‌شفاف.

خروجی باید علاوه بر score، critical pairها و عوامل score را ارائه کند.

### 21.7 Isovist

2D isovist:

- محاسبه روی plane با range؛
- polygon visibility؛
- area، perimeter، compactness، occlusivity و radial min/max/mean؛
- handling obstacleهای coplanar و gap tolerance.

3D isovist در 1.0 sampled است و نباید به‌عنوان exact volume معرفی شود مگر الگوریتم exact جداگانه پیاده شود.

### 21.8 annual daylight

metricهای sDA، ASE و UDI باید config versioned داشته باشند. threshold، occupancy schedule، sensor height، grid، material و engine settings در report ثبت می‌شوند. Fast Path فقط برآورد است؛ Validated Path مسئول نتیجه قابل مقایسه با workflow مرجع است.

- preset استاندارد نباید فقط با نام `sDA` یا `ASE` ذخیره شود؛ designation استاندارد، نسخه و پارامترها لازم‌اند.
- مرجع استاندارد جاری در زمان تدوین سند `ANSI/IES LM-83-23` است؛ implementation باید متن استاندارد دارای مجوز را در design review بررسی کند و از نسخه‌های قدیمی بدون label استفاده نکند.
- notationهایی مانند `sDA300,50%` باید threshold illuminance و fraction occupied hours را در داده نیز ذخیره کنند، نه فقط در label.
- ASE باید illuminance threshold، hour threshold، daily schedule و shading-device state را صریح ذخیره کند.
- UDI یک preset واحد جهانی فرض نمی‌شود؛ lower/upper bounds در config و نام خروجی می‌آیند.
- compliance presetها از metric general-purpose جدا هستند تا تغییر یک certification، semantics داده قدیمی را عوض نکند.

### 21.9 uncertainty

منابع uncertainty حداقل:

- sampling؛
- weather file؛
- sky model؛
- geometry discretization؛
- material assumptions؛
- numeric precision؛
- fast/reference model difference.

نسخه 1.0 باید دست‌کم sampling convergence و fast-vs-reference difference را گزارش کند. ادعای confidence interval رسمی فقط با روش آماری معتبر مجاز است.

---

## 22. backendهای محاسباتی

### 22.1 قرارداد Backend

backend interface مفهومی:

```text
capabilities()
build_scene(scene_description, build_config) -> BackendScene
update_scene(previous, delta) -> BackendScene
trace_any(scene, rays, filters) -> VisibilityBuffer
trace_closest(scene, rays, filters) -> HitBuffer
trace_target(scene, rays, target_filter) -> TargetBuffer
memory_stats()
cancel(job)
```

capabilityها شامل precision، query types، max buffer، device memory، async و supported material model هستند.

### 22.2 CPU Backend

الزامات:

- reference production backend و fallback عمومی؛
- parallelism با thread pool کنترل‌شده؛
- عدم اشغال بی‌حد همه coreها در Rhino؛
- user-configurable thread limit؛
- SIMD پس از benchmark و با scalar fallback؛
- f64 reference path برای validation منتخب؛
- deterministic reduction؛
- NUMA awareness در صورت نیاز آینده؛
- benchmark جدا برای build و traversal.

### 22.3 Portable GPU Backend

الزامات:

- `wgpu`/WGSL یا فناوری معادل قابل‌حمل؛
- compute-based traversal که به hardware RT experimental وابسته نباشد؛
- buffer chunking؛
- asynchronous upload/readback؛
- pipeline cache در صورت امکان؛
- device lost handling؛
- timestamp query فقط با capability check؛
- shader validation در CI و دستگاه واقعی؛
- fallback برای limitهای پایین adapter؛
- نتایج differential در برابر CPU.

**وضعیت 0.10.0:** subsystem جدید `VAYU` با `wgpu 30.0.1` و WGSL تحویل شد. implementation شامل snapshot مستقل origin-rebased f32 با اندازه‌گیری خطای واقعی quantization، conservative bounds، global 16-bin SAH BVH، closest/any-hit shader با stack محدود، exact 64-bit attribution packing، clamp به adapter binding/workgroup limits، deterministic ray chunking، ordered readback، adapter-name filtering و fallback صریح است. Rust/C/.NET/CLI/Rhino 8/9 پوشش end-to-end دارند. smoke differential واقعی روی NVIDIA RTX 5060 و Intel Raptor Lake/Vulkan با صفر state/identity mismatch ثبت شده است. در این نسخه upload/readback در API همگام public بسته‌بندی شده، pipeline cache پیاده نشده و geometry snapshot instance-expanded است؛ بنابراین WP-08/Gate 4 هنوز برای performance corpus، device-loss fault injection و two-level GPU instancing کاملاً بسته اعلام نمی‌شود.

**وضعیت 0.11.0:** قرارداد backend-neutral برای domain execution تحویل شد. DAENA Target/Weighted/Green View، protected Corridor و Dynamic Observer Path از یک implementation علمی مشترک و batchهای مرتب استفاده می‌کنند؛ فقط closest-hit executor میان CPU f64 و VAYU عوض می‌شود. provenance تجمیعی backend/adapter/batch/ray/dispatch/transfer/precision/fallback در Rust، C ABI، .NET 8/10، Rhino 8/9 و CLI حفظ می‌شود. `--verify-cpu` برای هر سه workflow مرجع f64 را اجرا کرده و روی numeric delta و discrete identity/state به‌صورت fail-closed gate اعمال می‌شود. async overlap، persistent mapped buffers، device-loss injection و BLAS/TLAS دو‌سطحی همچنان باز هستند.

**وضعیت 0.12.0:** persistent GPU buffers، queue-write upload، double-slot batched submission/readback و یک poll window برای هر دو dispatch تحویل شدند. API عمومی همگام باقی می‌ماند، اما overlap داخلی bounded شده است. `ComputeRuntimeStats` تعداد trace/ray/dispatch/reuse/fallback، نسل و ظرفیت arena، بایت‌های انتقال، device loss و آخرین خطا را بدون reset گزارش می‌کند. disk pipeline cache به‌علت نیاز API فعلی wgpu به `unsafe` و قرارداد `forbid(unsafe_code)` عمداً وارد نشده؛ fault injection واقعی driver و GPU BLAS/TLAS دو‌سطحی همچنان gateهای باز هستند.

**وضعیت 0.13.0:** cache دادهٔ portable plan بدون ذخیرهٔ pipeline blob درایور و بدون نقض `forbid(unsafe_code)` تحویل شد. cache شامل schema/magic/key/length/checksum، atomic rename، LRU memory، eviction دیسک و corruption recovery است. GPU BLAS/TLAS دو‌سطحی و geometry streaming نیز بسته شده‌اند؛ fault injection واقعی درایور همچنان validation سخت‌افزاری مستقل است و جزء ادعای completion نرم‌افزاری نیست.

### 22.4 NVIDIA Specialized Backend

این backend فقط در صورت گذراندن RFC و benchmark وارد 1.0 می‌شود. گزینه‌ها CUDA/OptiX یا API hardware ray tracing مناسب‌اند. شروط:

- مزیت معنادار نسبت به portable backend؛
- package قابل نصب و license سازگار؛
- isolation dependency؛
- feature parity مشخص؛
- عدم تبدیل شدن به شرط استفاده از محصول؛
- تست driver matrix؛
- fallback بدون data loss.

### 22.5 Radiance Backend/Bridge

Radiance backend برای query عمومی جایگزین core نیست؛ برای daylight validation استفاده می‌شود. bridge باید:

- input را deterministic تولید کند؛
- executable/version را ثبت کند؛
- command و parameters را در manifest نگه دارد؛
- job directory مستقل بسازد؛
- stdout/stderr را capture کند؛
- cancellation/timeout داشته باشد؛
- result mapping به SensorId را validate کند؛
- license و attribution Radiance را رعایت کند.

### 22.6 Backend selection

حالت‌ها:

- `Auto`؛
- `CPU`؛
- `PortableGPU(adapter_id)`؛
- `NvidiaSpecialized`؛
- `Reference`؛
- `RadianceValidated` برای metricهای مجاز.

`Auto` باید تصمیم و دلیل آن را report کند. تغییر خودکار backend در میانه job بدون ثبت ممنوع است.

---

## 23. cache، scheduler و concurrency

### 23.1 سطوح cache

1. adapter extraction cache؛
2. normalized mesh cache؛
3. BLAS cache؛
4. TLAS/scene cache؛
5. ray set/cache برای SunSet/SkySet؛
6. intermediate visibility matrix؛
7. metric result cache؛
8. report artifact cache.

هر سطح key، version و eviction policy مستقل دارد.

### 23.2 cache policy

- memory cache با LRU و budget؛
- disk cache opt-in/default قابل تنظیم؛
- atomic write و checksum؛
- corrupted cache باید حذف/ignore شود، نه crash؛
- cache حاوی path یا داده حساس باید local باشد؛
- UI امکان inspect و clear در scope مشخص دارد؛
- eviction نباید file کاربر را حذف کند؛ فقط directory managed XVARNA؛
- cache schema migration یا invalidation صریح است.

### 23.3 scheduler

- jobها priority دارند: interactive، user-run، batch، background validation؛
- interactive job می‌تواند batch کم‌اولویت را pause/cancel کند؛
- GPU queue و CPU pool هماهنگ اما مستقل‌اند؛
- duplicate identical jobs coalesce می‌شوند؛
- stale jobs هنگام تغییر input cancel یا mark stale می‌شوند؛
- Grasshopper solution lifecycle رعایت می‌شود؛
- completion callback نباید UI thread را با پردازش سنگین بلوکه کند.

### 23.4 cancellation latency

هدف:

- CPU batch: مشاهده cancellation حداکثر هر chunk کوتاه؛
- GPU: بین dispatchها؛ dispatch فوق‌العاده طولانی ممنوع؛
- Radiance: terminate process tree کنترل‌شده؛
- report: بین stageها.

partial result فقط اگر semantic معتبر و label `Partial` داشته باشد برگردانده می‌شود.

### 23.5 reproducibility و parallel reduction

- reductionهای floating-point ممکن است با ترتیب thread تفاوت کنند؛ روش deterministic برای Q3/Q4 لازم است.
- Q1 می‌تواند reduction سریع‌تر داشته باشد، اما error bound و nondeterminism محدود باید ثبت شود.
- random sampling از streamهای seedشده و indexable استفاده می‌کند تا تعداد thread نتیجه را عوض نکند.

---

## 24. FFI و API عمومی

### 24.1 اصول C ABI

- ABI با C سازگار و versioned است.
- هیچ `Vec`, `String`, trait object یا pointer متعلق به Rust بدون wrapper عبور نمی‌کند.
- ownership هر buffer در نام API و مستندات روشن است.
- handleها opaque و generation-checked هستند.
- تمام functionها error code برمی‌گردانند یا result struct استاندارد دارند.
- panic نباید از boundary عبور کند؛ capture و تبدیل می‌شود.
- thread-safety هر handle مستند است.
- structها `struct_size` و `api_version` دارند تا توسعه سازگار ممکن شود.

### 24.2 الگوی API

نمونه مفهومی، نه signature نهایی:

```c
xv_status xv_context_create(const xv_context_desc*, xv_context** out);
xv_status xv_scene_create(xv_context*, const xv_scene_desc*, xv_scene** out);
xv_status xv_scene_update(xv_scene*, const xv_scene_delta*, xv_scene** out);
xv_status xv_job_submit(xv_context*, const xv_job_desc*, xv_job** out);
xv_status xv_job_poll(xv_job*, xv_job_state* out);
xv_status xv_job_cancel(xv_job*);
xv_status xv_job_get_result(xv_job*, xv_result** out);
void      xv_result_release(xv_result*);
const xv_error* xv_get_last_error(xv_context*);
```

### 24.3 error taxonomy

گروه‌های error:

- `XV_E_INVALID_ARGUMENT`؛
- `XV_E_INVALID_GEOMETRY`؛
- `XV_E_UNSUPPORTED_CAPABILITY`؛
- `XV_E_OUT_OF_MEMORY`؛
- `XV_E_DEVICE_LOST`؛
- `XV_E_CANCELLED`؛
- `XV_E_IO`؛
- `XV_E_SCHEMA`؛
- `XV_E_REFERENCE_ENGINE`؛
- `XV_E_INTERNAL`؛
- `XV_E_PANIC_CAPTURED`.

error شامل code، message، subsystem، context fields، cause chain و remediation hint است. اطلاعات حساس path در log shareable redact می‌شود.

### 24.4 API compatibility

- C ABI با semantic versioning؛
- schema با major/minor و migration؛
- Rust API قبل از 1.0 unstable؛
- .NET wrapper API پس از beta freeze؛
- component GUIDهای Grasshopper پس از اولین beta عمومی/دعوتی قفل می‌شوند تا فایل‌های GH نشکنند؛
- rename component باید alias/migration داشته باشد.

### 24.5 Python API

Python binding P1/P2 است و نباید هسته را به نسخه Python خاص قفل کند. wheelهای رسمی فقط برای نسخه‌هایی منتشر می‌شوند که CI و dependencyها پشتیبانی می‌کنند. Python 3.14 دستگاه توسعه لزوماً target اولیه package نیست.

---

## 25. معماری افزونه Rhino/Grasshopper و UX

### 25.1 نقش لایه C#

لایه C# باید نازک بماند و فقط وظایف زیر را انجام دهد:

- extraction هندسه و metadata از RhinoCommon؛
- mapping Data Tree؛
- مدیریت lifecycle کامپوننت و document؛
- async UI integration؛
- preview، conduit و selection؛
- تبدیل typeهای .NET به bufferهای FFI؛
- نمایش error/help؛
- packaging و settings UI.

محاسبات domain، BVH و metricهای اصلی نباید در C# duplicate شوند.

### 25.2 lifecycle صحنه در Grasshopper

- `XV Scene` handle وابسته به document/session دارد.
- handle پس از بسته‌شدن document آزاد می‌شود.
- document copy/save-as نباید collision cache ایجاد کند.
- recompute بدون تغییر content cache hit می‌دهد.
- تغییر display-only موجب rebuild نمی‌شود.
- اگر Rhino object reference تغییر کرد، object GUID/content hash بررسی می‌شود.
- stale handle باید با پیام قابل‌فهم reject یا refresh شود.

### 25.3 async component behavior

state machine پیشنهادی:

```text
Idle → Validating → Queued → Building/Uploading → Running
     → Reducing → Completed
     → Cancelled | Failed | Stale
```

UI باید state را بدون spam پیام نمایش دهد. تغییر input job قبلی را طبق debounce/cancel policy مدیریت می‌کند. output آخر می‌تواند با watermark `stale` باقی بماند تا viewport خالی نشود.

### 25.4 debounce

- slider drag نباید صدها job queue کند.
- default debounce برای interactive component قابل تنظیم است.
- component می‌تواند حالت `Manual`, `Debounced`, `OnSolution` داشته باشد.
- محاسبات بسیار سنگین در Q3/Q4 default به `Manual` هستند.
- debounce time در expert settings و diagnostics ثبت می‌شود.

### 25.5 preview و visualization

الزامات:

- GPU-friendly colored mesh/point preview؛
- legend یکپارچه و قابل bake؛
- paletteهای perceptually uniform؛
- palette مناسب color-vision deficiency؛
- clamp/outlier handling آشکار؛
- log/linear scale انتخابی؛
- NaN/invalid/stale color جدا؛
- selection sensor و highlight occluder؛
- نمایش timeline و tooltip جزئیات؛
- جلوگیری از duplicate geometry سنگین در Grasshopper memory؛
- level-of-detail برای point cloud/grid بزرگ.

### 25.6 UX سطح مبتدی و حرفه‌ای

**Basic Mode:**

- ورودی‌های ضروری؛
- presetهای امن؛
- پیام ساده؛
- help تصویری؛
- advanced inputs پنهان اما قابل دسترسی.

**Expert Mode:**

- tolerance؛
- sampling؛
- backend/device؛
- memory budget؛
- sky model؛
- metric config؛
- seed؛
- diagnostics؛
- export manifest.

Basic Mode نباید فرض‌ها را مخفی کند؛ summary آن‌ها در tooltip/report باقی می‌ماند.

### 25.7 Data Tree policy

- branchها تا حد ممکن ساختار SensorSet/variant را حفظ می‌کنند.
- mapping ورودی به output مستند و deterministic است.
- flatten/graft پنهان ممنوع است.
- mismatch طول listها error روشن دارد.
- broadcasting فقط در الگوهای مستند انجام می‌شود.
- result بزرگ می‌تواند object handle برگرداند و component جدا برای استخراج subset داشته باشد تا canvas منفجر نشود.

### 25.8 پیام‌ها

سطوح:

- `Info`: انتخاب backend، cache hit؛
- `Remark`: approximation یا setting غیرمعمول؛
- `Warning`: geometry issue، fallback، uncertainty بالا؛
- `Error`: نتیجه نامعتبر یا job failed.

پیام باید بگوید چه اتفاقی افتاده، چرا مهم است و کاربر چه کاری می‌تواند انجام دهد. stack trace در UI عادی نمایش داده نمی‌شود، اما در diagnostic bundle وجود دارد.

### 25.9 accessibility و localization

- UI اصلی 1.0 انگلیسی است تا انتشار بین‌المللی ممکن باشد.
- زیرساخت localization از ابتدا در نظر گرفته می‌شود؛ string hard-code در logic ممنوع.
- مستندات فارسی می‌تواند همراه نسخه انگلیسی منتشر شود.
- keyboard navigation و contrast در panelهای custom رعایت می‌شود.
- واحد، decimal separator و date formatting مستقل از parsing canonical هستند.

---

## 26. فرمت فایل، schema و interoperability

### 26.1 اصول schema

- JSON Schema برای manifestهای انسانی؛
- binary/columnar format برای bufferهای بزرگ؛
- schema version در root؛
- unknown field در minor version قابل ignore؛
- breaking field در major جدید؛
- migration tool برای حداقل یک major قبلی در آینده؛
- hash artifact و referenced fileها ثبت می‌شود.

### 26.2 Study Manifest

`XvarnaStudyManifest` حداقل:

- schema/version؛
- study ID/title/author؛
- creation/update time؛
- scene references و hashes؛
- variants و parameter table؛
- weather/time config؛
- sensor/target definitions؛
- metrics و quality configs؛
- backend preference؛
- result references؛
- validation references؛
- license/provenance؛
- notes/tags.

### 26.3 Result storage

- metadata کوچک در JSON؛
- array بزرگ در Parquet/Arrow-like یا binary versioned؛
- ordering و ID mapping صریح؛
- compression قابل انتخاب؛
- checksum per artifact؛
- partial/cancelled state؛
- عدم وابستگی reader به Rhino.

### 26.4 formatهای import/export 1.0

| Format | Import | Export | وضعیت |
|---|---|---|---|
| Rhino geometry via adapter | بله | bake/reference | P0 |
| OBJ | بله | اختیاری | P1 |
| STL | بله | اختیاری | P1 |
| PLY | بله | بله | P1 |
| glTF/GLB | بله | بله با color/metadata منتخب | P1 |
| CSV | weather/sensor/config محدود | result/time series | P0 |
| JSON | manifest/config | manifest/result metadata | P0 |
| Parquet | study/result | study/result | P1 |
| EPW | بله | خیر | P0 |
| WEA | بله | بله برای bridge | P1 |
| Radiance scene files | از طریق bridge | بله | P1 |
| IFC | مستقیم خیر؛ از Rhino/convert | خیر | خارج از 1.0 |

### 26.5 provenance

هر artifact باید امکان پاسخ به این سؤال‌ها را بدهد:

- با کدام نسخه engine ساخته شد؟
- ورودی دقیق چه hashی داشت؟
- چه تنظیماتی و چه quality tierی استفاده شد؟
- روی چه backend/device/driverی اجرا شد؟
- چه زمانی و با چه seedی؟
- آیا warning یا fallback رخ داد؟
- آیا نتیجه validated است؟
- license و منبع dataset چیست؟

---

## 27. logging، diagnostics و observability

### 27.1 logging

- structured logging با event ID؛
- levelهای trace/debug/info/warn/error؛
- session/job correlation ID؛
- rolling local logs با size limit؛
- path و نام پروژه در bundle عمومی قابل redact؛
- logging hot path default خاموش یا sampled؛
- هیچ geometry خام در log نوشته نمی‌شود مگر opt-in diagnostic صریح.

### 27.2 telemetry

telemetry اجباری ممنوع است. اگر در آینده telemetry opt-in اضافه شد:

- default خاموش؛
- schema عمومی؛
- preview داده ارسالی؛
- هیچ geometry/path/project name؛
- امکان export محلی و حذف؛
- privacy policy روشن.

### 27.3 diagnostic bundle

bundle قابل ارسال برای issue:

- version/capabilities؛
- OS/CPU/GPU/driver summary؛
- plugin list منتخب و نسخه‌ها در صورت مجاز بودن؛
- log redacted؛
- config و result header؛
- error chain؛
- optional minimal reproducer فقط با رضایت کاربر؛
- checksum.

### 27.4 performance trace

هر job stage timing دارد:

- extraction؛
- validation؛
- hash/cache lookup؛
- BLAS build/load؛
- TLAS build/refit؛
- upload؛
- ray generation؛
- traversal؛
- reduction؛
- readback؛
- visualization/report.

این breakdown برای یافتن bottleneck و جلوگیری از ادعای ناقص ضروری است.

---

## 28. امنیت، حریم خصوصی و پایداری

### 28.1 threat model سطح 1.0

ورودی‌های بالقوه غیرقابل اعتماد:

- فایل mesh؛
- EPW/CSV/JSON؛
- cache قدیمی/خراب؛
- shader/config؛
- path خروجی؛
- Radiance executable/path؛
- report template؛
- package update metadata.

### 28.2 الزامات امنیتی

| ID | نیازمندی | اولویت |
|---|---|---|
| `SEC-001` | parserها bound و size limit داشته باشند | P0 |
| `SEC-002` | arithmetic overflow و index check | P0 |
| `SEC-003` | path traversal prevention در archive/report | P0 |
| `SEC-004` | اجرای process بدون shell interpolation ناامن | P0 |
| `SEC-005` | dependency audit و lockfile | P0 |
| `SEC-006` | release artifact checksum/signing | P1 |
| `SEC-007` | SECURITY.md و disclosure process | P0 |
| `SEC-008` | memory ownership تست‌شده در FFI | P0 |
| `SEC-009` | panic/exception boundary | P0 |
| `SEC-010` | cache directory scoped و validated | P0 |

### 28.3 process execution

Radiance و ابزارهای خارجی با argument array امن اجرا می‌شوند؛ command string ساخته و به shell داده نمی‌شود. working directory اختصاصی است. timeout، cancellation و process tree cleanup اجرا می‌شوند. executable path و hash در manifest ثبت می‌شوند.

### 28.4 privacy

- همه محاسبات اصلی local هستند.
- report فقط با فرمان کاربر share می‌شود.
- web version برای فایل local باید امکان پردازش client-side را اولویت دهد.
- اگر backend server در آینده اضافه شد، consent و retention policy مستقل لازم است.
- نمونه پروژه‌های beta بدون اجازه کتبی منتشر نمی‌شوند.

### 28.5 crash containment

- Rust panic در FFI capture؛
- GPU device lost به error/fallback؛
- out-of-memory با budget check پیشگیرانه؛
- report/export با write موقت و rename atomic؛
- Rhino document نباید به‌علت failure engine corrupt شود؛
- auto-save یا source geometry هرگز توسط engine overwrite نمی‌شود.

---

## 29. بودجه‌های performance و حافظه

### 29.1 workloadهای مرجع

تعریف نهایی dataset پس از ساخت harness freeze می‌شود. کلاس اولیه:

| کلاس | مثلث | sensor | direction/time | کاربرد |
|---|---:|---:|---:|---|
| S | 100k | 10k | 145 | ساختمان/آموزش |
| M | 1M | 100k | 145–8760 sparse/weighted | سایت شهری |
| L | 5M | 250k | batch/chunked | مدل بزرگ دفتر |
| XL | 20M+ | 1M | chunked | stress/out-of-core، نه الزام interactive کامل |

اعداد dataset دقیق، هندسه و query count باید manifest داشته باشند. تعداد raw rays مشتق‌شده و rays actually traced هر دو گزارش می‌شوند.

### 29.2 بودجه دستگاه مرجع

اهداف اولیه روی i7-14650HX + RTX 5060 Laptop + 16 GB RAM:

- S/Q1 warm-cache: P95 کل update زیر 0.5 ثانیه؛
- M/Q1 برای metric منتخب: P95 زیر 1 ثانیه در workload دقیقاً تعریف‌شده؛
- scene build M: هدف زیر چند ثانیه با progress؛
- idle memory plugin بدون scene: هدف کمتر از 250 MB اضافه؛
- memory overhead acceleration: هدف قابل رقابت و کمتر از چند برابر raw geometry؛ مقدار دقیق پس از layout benchmark؛
- cancellation: احساس پاسخ‌گویی UI حداکثر حدود 200 ms برای درخواست، حتی اگر GPU dispatch جاری کمی دیرتر پایان یابد.

این اهداف aspirational و go/no-go مهندسی‌اند؛ تا قبل از dataset freeze ادعای عمومی نیستند.

### 29.3 performance correctness

- benchmark release build، بدون debugger و با warm/cold تفکیک؛
- clock/thermal state ثبت شود؛
- حداقل چند repetition و statistic مناسب؛
- median، P95 و dispersion؛
- build time و query time جدا؛
- upload/readback و visualization جدا؛
- cache hit/miss جدا؛
- quality/accuracy ثابت؛
- نتایج نامعتبر از performance leaderboard حذف نشوند؛ به‌عنوان failure ثبت شوند.

### 29.4 memory budget

کاربر می‌تواند CPU RAM و GPU VRAM budget تعیین کند. engine باید قبل از allocation بزرگ estimate بدهد. اگر بیش از budget است:

1. compact layout؛
2. chunking؛
3. کاهش diagnostic payload؛
4. fallback backend؛
5. error actionable.

کاهش sample یا quality بدون اجازه کاربر ممنوع است.

---

## 30. Validation، Verification و Benchmark

### 30.1 تفکیک مفاهیم

- **Verification:** آیا الگوریتم مطابق تعریف خودش پیاده شده است؟
- **Validation:** آیا مدل برای پدیده مورد نظر به مرجع مناسب نزدیک است؟
- **Benchmark:** با چه هزینه زمانی/حافظه‌ای اجرا می‌شود؟
- **Regression:** آیا نسخه جدید نتیجه یا performance را ناخواسته خراب کرده است؟

### 30.2 validation ladder

1. مسائل هندسی تحلیلی؛
2. brute-force/reference CPU؛
3. differential CPU در برابر GPU؛
4. sceneهای مصنوعی کنترل‌شده؛
5. مقایسه با Rhino/MeshRay برای visibility؛
6. مقایسه با Radiance برای daylight/radiation منتخب؛
7. case study واقعی؛
8. مطالعه کاربری.

### 30.3 analytic corpus

حداقل شامل:

- plane بدون مانع؛
- wall با shadow boundary معلوم؛
- box/courtyard؛
- slit و narrow opening؛
- two touching triangles؛
- coplanar triangles؛
- thin geometry؛
- nested instances؛
- mirrored transform؛
- origin دور؛
- scene با scale میلی‌متر و کیلومتر؛
- sun positionهای مرزی sunrise/sunset؛
- hemisphere با occlusion کسری قابل محاسبه؛
- target با solid angle تحلیلی؛
- privacy pair با فاصله/زاویه کنترل‌شده.

### 30.4 differential testing

برای مجموعه rayهای deterministic:

- CPU reference f64؛
- CPU production؛
- portable GPU؛
- specialized GPU در صورت وجود.

مقایسه:

- hit/miss؛
- distance؛
- object/triangle ID؛
- aggregate metric؛
- top-K attribution؛
- runtime/memory.

اختلاف edge case باید با policy شناخته‌شده classify شود، نه فقط tolerance بزرگ.

### 30.5 Radiance validation

ماتریس validation حداقل شامل:

- فضای ساده با opening؛
- اتاق با reflectanceهای مختلف؛
- سایبان خارجی؛
- courtyard؛
- مدل اداری نمونه؛
- skyهای منتخب؛
- point-in-time و annual metric؛
- fast mode در برابر validated mode.

report باید absolute error، relative error، bias، percentile و spatial map اختلاف را ارائه کند. metric صفر/نزدیک صفر با relative error گمراه‌کننده جداگانه تحلیل می‌شود.

### 30.6 absolute benchmark and reproducibility suite

هر benchmark رسمی XVARNA حداقل این اطلاعات را ثبت می‌کند:

- نسخه engine/connector؛
- تاریخ؛
- hardware؛
- dataset و input conversion؛
- quality setting؛
- warm/cold؛
- command/workflow؛
- raw result؛
- failure/crash؛
- محدودیت تفسیر نتیجه.

اسکریپت و داده تا حد مجاز منتشر می‌شوند. مطالعه محصولات دیگر برای feature discovery در یک market matrix داخلی و تاریخ‌دار نگهداری می‌شود، اما اجرای head-to-head یا ادعای برتری شرط release نیست.

### 30.7 performance regression gates

- microbenchmark در CI؛
- macrobenchmark nightly یا دستگاه اختصاصی؛
- threshold جدا برای noise؛
- regression بیش از حد بدون label مانع merge؛
- baseline همراه commit و hardware profile؛
- performance improvement بدون correctness pass پذیرفته نیست.

### 30.8 معیار پذیرش اولیه metricها

| metric | معیار حداقلی |
|---|---|
| visibility binary | تطابق بیش از 99.9% روی corpus غیرمبهم؛ edge ambiguity جدا |
| hit distance | tolerance ترکیبی absolute/relative تعریف‌شده |
| Direct Sun Hours | تطابق کامل timeline روی analytic cases غیرمرزی |
| SVF | convergence و error زیر threshold dataset-specific |
| Target View | تطابق با reference sampling/exact cases |
| daylight Fast | error budget منتشرشده؛ عدم ادعای validated |
| Radiance Path | reproducibility و mapping کامل SensorId |
| attribution | مجموع سهم‌ها + unassigned برابر total blocked weight |

---

## 31. راهبرد تست

### 31.1 لایه‌های تست

| لایه | هدف | نمونه |
|---|---|---|
| Unit | منطق محلی | vector math، parser، weight function |
| Property-based | invariantها | BVH bounds، serialization roundtrip |
| Differential | مقایسه implementation | CPU reference vs GPU |
| Integration | اتصال crate/FFI | scene → job → result |
| Contract | ABI/schema/API | struct size، version، backward read |
| Component | Grasshopper behavior | Data Tree، cancellation، stale output |
| End-to-end | workflow کامل | EPW + Rhino model → report |
| Golden | artifact/result پایدار | analytic scenes، report snapshots |
| Fuzz | parser/FFI robustness | OBJ/JSON/EPW malformed |
| Stress | scale و پایداری | میلیون‌ها triangle/ray |
| Performance | regression | build/traversal/reduction |
| Usability | درک کاربر | onboarding و task completion |

### 31.2 invariantهای هسته

- همه primitive bounds داخل ancestor bounds هستند.
- هیچ leaf index خارج range نیست.
- hit distance در `[t_min, t_max]` است.
- direction/result buffer lengthها سازگارند.
- object/instance mapping roundtrip می‌شود.
- serialization و deserialization hash semantic را حفظ می‌کند.
- مجموع attribution با total blocked weight سازگار است.
- result مربوط به scene revision اشتباه قابل استفاده نیست.
- cancellation resource leak ایجاد نمی‌کند.
- repeated create/release handle memory رشد بی‌حد ندارد.

### 31.3 fuzzing

هدف‌ها:

- mesh importers؛
- EPW/CSV/JSON parser؛
- schema migration؛
- FFI length/pointer validation؛
- cache reader؛
- report archive؛
- shader input range از host.

crash، timeout غیرعادی، OOM ناشی از size field و undefined behavior failure محسوب می‌شوند.

### 31.4 FFI safety testing

- null pointer و zero length؛
- non-null pointer با length نامعتبر در harness کنترل‌شده؛
- double release؛
- use-after-release handle generation check؛
- concurrent calls؛
- callback lifetime؛
- panic injection؛
- allocation failure simulation؛
- ABI layout test میان Rust/C/C#.

### 31.5 Grasshopper tests

- باز کردن example بدون missing component؛
- recompute؛
- cancel؛
- document close هنگام job؛
- duplicate component؛
- copy/paste؛
- undo/redo؛
- save/reopen؛
- plugin update؛
- missing native library؛
- unsupported GPU؛
- driver failure؛
- Data Tree edge cases؛
- unit change؛
- large preview؛
- coexistence با pluginهای پرکاربرد منتخب.

### 31.6 test data governance

- هر dataset دارای `LICENSE`, `SOURCE`, `HASH`, `PURPOSE` است.
- پروژه client بدون اجازه وارد corpus نمی‌شود.
- geometry تولیدی procedural ترجیح دارد تا edge case بازتولیدپذیر باشد.
- dataset بزرگ با version و checksum دانلود می‌شود.
- تغییر golden فقط با review توضیح‌دار انجام می‌شود.

### 31.7 استانداردهای کدنویسی

**Rust:**

- edition 2024 مگر RFC خلاف آن؛
- `rustfmt` و lintهای CI؛
- warning در crateهای production به‌عنوان error در CI؛
- `unsafe` فقط در module محدود با `SAFETY` comment، test و reviewer؛
- `unwrap/expect` در مسیر ورودی، FFI و runtime production ممنوع؛ فقط invariant اثبات‌شده با توضیح؛
- arithmetic حساس با checked/saturating policy صریح؛
- allocation در hot loop فقط پس از profiling و در حد کنترل‌شده؛
- public item دارای rustdoc؛
- feature flagها additive و تست‌شده؛
- dependency جدید با دلیل، license، maintenance و benchmark size بررسی می‌شود.

**C#/.NET:**

- nullable reference types فعال؛
- analyzerها و warnings as errors برای پروژه اصلی؛
- dispose/lifetime صریح برای native handle؛
- UI thread access محدود و مستند؛
- exception native به typeهای مشخص ترجمه شود؛
- logic domain در adapter duplicate نشود؛
- component GUID ثابت و در test ثبت شود.

**WGSL/GPU:**

- buffer layout در host/shader با contract test؛
- magic number بدون constant/config ممنوع؛
- bounds check طبق threat/performance analysis؛
- shader variant explosion کنترل شود؛
- timestamp/profile code از correctness code جدا؛
- هر optimization غیر obvious با comment و benchmark reference.

**عمومی:**

- commit/PR کوچک و قابل review؛
- نام‌گذاری انگلیسی و یکنواخت؛
- comment علت را توضیح دهد، نه تکرار کد؛
- هر workaround issue و expiry condition دارد؛
- dead code و feature متروک پیش از release حذف می‌شود.

---

## 32. CI/CD و مهندسی انتشار

### 32.1 CI برای هر PR

- format/lint Rust و .NET؛
- build workspace؛
- unit/property/contract tests؛
- schema validation؛
- shader validation؛
- license header/dependency policy؛
- docs link check منتخب؛
- small benchmark smoke؛
- artifact native برای target اصلی؛
- FFI integration test؛
- security auditهای قابل اجرا.

### 32.2 nightly

- full test matrix؛
- fuzz budget؛
- large benchmark؛
- GPU differential روی دستگاه runner واقعی؛
- memory/leak soak؛
- package install smoke؛
- Radiance validation subset؛
- example open/run؛
- report generation.

### 32.3 release pipeline

1. version freeze؛
2. dependency freeze؛
3. changelog و migration؛
4. full clean build؛
5. test/benchmark/validation؛
6. SBOM؛
7. artifact signing/checksum؛
8. Yak/package install روی سیستم تمیز؛
9. antivirus false-positive check؛
10. documentation snapshot؛
11. release candidate به beta group؛
12. go/no-go review؛
13. Git tag و immutable release؛
14. DOI archive؛
15. website/docs publish؛
16. announcement.

### 32.4 versioning

- product: Semantic Versioning؛
- schema: نسخه مستقل اما map‌شده به release؛
- metric definition: ID + semantic version؛
- benchmark suite: dataset/harness version؛
- shader/cache binary: internal format version؛
- Grasshopper component GUID ثابت؛
- pre-1.0 internal buildها `0.x.y-internal` یا commit SHA.

### 32.5 supply chain

- dependency pin و review؛
- حداقل dependencyهای native؛
- SBOM برای release؛
- checksum تمام binaryها؛
- provenance CI در صورت امکان؛
- ممنوعیت download/execute کد ناشناس در runtime؛
- update check فقط metadata و opt-in/قابل خاموش‌کردن؛
- binary third-party همراه license/NOTICE.

### 32.6 build profileها

- `dev`: compile سریع و assertions؛
- `test`: instrumentation لازم؛
- `bench`: optimized با debug symbols منتخب؛
- `release`: LTO/codegen settings پس از benchmark، panic strategy سازگار با FFI؛
- `reference`: precision/checkهای بیشتر، نه performance؛
- `sanitized`: در targetهای ممکن برای ASan/UBSan یا ابزار معادل dependencyهای native.

تنظیم release نباید قبل از profiling پیچیده شود. تغییر compiler flag با benchmark و correctness gate انجام می‌شود.

---

## 33. مستندات و آموزش

### 33.1 مجموعه مستندات اجباری

- README محصول؛
- Installation؛
- Quick Start؛
- Concepts؛
- Component Reference؛
- Metric Definitions؛
- Accuracy and Limitations؛
- Backend and Hardware Guide؛
- Troubleshooting؛
- Developer Guide؛
- FFI/API Reference؛
- Benchmark Methodology؛
- Validation Report؛
- Research Reproduction Guide؛
- Contributing؛
- Security؛
- Governance؛
- Citation؛
- Release Notes.

### 33.2 tutorialهای اجباری

1. اولین Sun Hours؛
2. سایبان پارامتریک با update تعاملی؛
3. تحلیل Sky View در courtyard؛
4. View Quality به پارک و آسمان؛
5. Privacy بین دو نما؛
6. annual solar/radiation با EPW؛
7. Fast vs Radiance Validated daylight؛
8. Compare design options؛
9. استفاده در Galapagos/Wallacei؛
10. ساخت report و citation؛
11. CLI batch؛
12. توسعه یک metric/connector نمونه.

### 33.3 سطح مستندات component

هر component صفحه‌ای با موارد زیر دارد:

- هدف؛
- diagram workflow؛
- input/output با type/unit/tree semantics؛
- algorithm summary؛
- assumptions؛
- quality tiers؛
- performance notes؛
- warning/errorها؛
- example؛
- references؛
- version introduced/changed.

### 33.4 example quality

- example باید روی سیستم تمیز اجرا شود.
- asset license روشن باشد.
- output expected screenshot/value داشته باشد.
- زمان و سخت‌افزار تقریبی ثبت شود.
- example قدیمی در CI یا release smoke شناسایی شود.
- فایل بسیار بزرگ به‌صورت download با hash ارائه شود.

---

## 34. برنامه پژوهشی

### 34.1 سؤال پژوهشی اصلی

> آیا تحلیل ray-based با scene update افزایشی، backendهای ناهمگون و attribution صریح می‌تواند بازخورد محیطی و فضایی را در زمان طراحی پارامتریک سریع‌تر، قابل‌فهم‌تر و قابل‌اعتمادتر کند؟

### 34.2 فرضیه‌ها

| ID | فرضیه |
|---|---|
| `H1` | caching و BLAS/TLAS update زمان iteration را نسبت به rebuild کامل به‌طور معنادار کاهش می‌دهد. |
| `H2` | portable GPU و CPU optimized روی workloadهای معماری throughput بالاتری از baseline CAD ray workflow دارند. |
| `H3` | attribution زمان تشخیص علت نتیجه نامطلوب را کاهش می‌دهد. |
| `H4` | نمایش uncertainty و validation trust calibration کاربر را بهبود می‌دهد. |
| `H5` | یک data model مشترک میان fast و validated path خطای انتقال workflow را کاهش می‌دهد. |

### 34.3 contributions هدف

- معماری acceleration/update برای design iteration؛
- مدل attribution شیء-زمان برای metricهای معماری؛
- reproducibility contract میان CPU/GPU/reference؛
- benchmark عمومی architectural ray queries؛
- workflow fast-to-validated؛
- شواهد usability و decision quality.

### 34.4 طراحی آزمایش performance

- independent variables: scene scale، sensor count، direction count، change type، backend؛
- dependent variables: build/update/query/total time، RAM/VRAM، energy تقریبی در صورت ابزار، accuracy؛
- control: hardware، version، thermal condition، quality؛
- repetitions و warm-up از پیش تعریف؛
- analysis شامل median/P95 و confidence مناسب؛
- raw data و script منتشر شوند.

### 34.5 مطالعه کاربری

گروه‌ها:

- دانشجو؛
- طراح محاسباتی؛
- متخصص محیطی.

taskها:

- یافتن عامل افت آفتاب؛
- اصلاح سایبان؛
- مقایسه دو massing؛
- شناسایی privacy conflict؛
- توضیح تصمیم.

شرایط مقایسه:

- ابزار baseline بدون attribution؛
- XVARNA با attribution؛
- در صورت امکان ترتیب counterbalanced.

metricها:

- task completion time؛
- accuracy پاسخ؛
- تعداد iteration؛
- confidence؛
- trust calibration؛
- usability score؛
- qualitative interview.

مطالعه انسانی نیازمند رضایت آگاهانه، anonymization و در صورت کاربرد approval اخلاق پژوهش است.

### 34.6 خروجی‌های علمی

- technical report الگوریتم و benchmark؛
- preprint open access؛
- dataset با DOI؛
- software release با DOI؛
- paper اصلی برای حوزه CAAD/computational design/building performance؛
- poster/demo؛
- thesis/portfolio chapter؛
- reproducibility package.

### 34.7 ساختار مقاله پیشنهادی

1. مسئله و شکاف؛
2. related work؛
3. system architecture؛
4. incremental scene/query؛
5. attribution model؛
6. fast/reference workflow؛
7. benchmark؛
8. validation؛
9. user study؛
10. limitations؛
11. open-source artifact.

---

## 35. مجوز، مالکیت فکری و governance

### 35.1 مجوز پیشنهادی

- source code اصلی: `Apache-2.0`؛
- documentation: `CC BY 4.0` یا مطابق تصمیم نهایی؛
- benchmark data تولیدی: `CC BY 4.0` یا `CC0` بسته به منبع؛
- third-party data: مطابق license اصلی؛
- نام و لوگوی XVARNA: trademark policy جدا، حتی اگر کد متن‌باز باشد.

Apache-2.0 به‌دلیل مجوز permissive و patent grant برای adoption دانشگاهی/صنعتی مناسب است. قبل از انتشار، سازگاری تمام dependencyها بررسی می‌شود.

### 35.2 سیاست contribution

- DCO sign-off یا CLA بر اساس بررسی حقوقی؛ ترجیح اولیه DCO برای سادگی؛
- Code of Conduct؛
- issue/PR templates؛
- review اجباری برای unsafe/FFI/shader/metric changes؛
- test و docs برای feature؛
- benchmark برای performance claim؛
- provenance برای algorithm/reference.

### 35.3 governance اولیه

تا قبل از تشکیل maintainer group، مؤسس BDFL/lead maintainer است. تصمیم‌های مهم در RFC عمومی ثبت می‌شوند. پس از رشد پروژه:

- maintainer role؛
- reviewer حوزه؛
- release manager؛
- security contact؛
- research/data steward.

### 35.4 citation

repository باید `CITATION.cff`، DOI release، preferred citation و BibTeX داشته باشد. report XVARNA باید citation engine و dataset را تولید کند.

### 35.5 علامت تجاری

متن‌باز بودن به معنی اجازه استفاده گمراه‌کننده از نام/لوگو نیست. policy باید اجازه fork را بدهد اما نسبت‌دادن نسخه تغییرکرده به release رسمی XVARNA را محدود کند. بررسی حقوقی نام پیش از launch الزامی است.

---

## 36. مدل جامعه، پشتیبانی و issue management

### 36.1 کانال‌ها

- GitHub Issues برای bug/feature؛
- GitHub Discussions برای Q&A/ideas؛
- documentation website؛
- McNeel Forum برای announcement و integration issues منتخب؛
- ایمیل امنیتی خصوصی؛
- هیچ پشتیبانی اصلی فقط در پیام خصوصی پراکنده انجام نمی‌شود.

### 36.2 severity

| Severity | تعریف | هدف پاسخ اولیه |
|---|---|---|
| S0 | امنیت/از‌دست‌رفتن داده/crash گسترده | فوری طبق توان تیم؛ release patch |
| S1 | نتیجه غلط بدون هشدار یا feature اصلی خراب | اولویت بالای patch |
| S2 | bug با workaround | برنامه 1.x |
| S3 | UX/docs/minor | backlog |

SLA حقوقی تضمین‌شده برای نسخه community وجود ندارد؛ جدول هدف عملیاتی است.

### 36.3 bug مربوط به correctness

bug نتیجه غلط از bug UI مهم‌تر است. برای correctness bug:

- affected versions؛
- affected metric/config؛
- reproducible case؛
- severity؛
- correction؛
- آیا نتایج قبلی باید invalidate شوند؟
- release note آشکار.

---

## 37. برنامه انتشار و launch

### 37.1 مراحل غیرعمومی

| مرحله | خروجی | شرط عبور |
|---|---|---|
| Engineering Preview | CLI/core داخلی | analytic correctness و profiling |
| Internal Alpha | GH core components | workflow end-to-end داخلی |
| Closed Alpha | 5–10 کاربر | نبود blocker بنیادی و feedback اولیه |
| Closed Beta 1 | 20+ کاربر | installation/UX/correctness قابل قبول |
| Closed Beta 2 / RC | feature freeze | zero P0، benchmark/validation draft |
| Public 1.0 | release کامل | تمام release gateها |

### 37.2 دارایی‌های launch

- GitHub repository کامل؛
- release binary و Yak package؛
- landing page؛
- documentation؛
- 60–90 ثانیه hero video؛
- ویدئوی technical deep dive؛
- سه case study؛
- public benchmark dashboard/report؛
- validation whitepaper؛
- preprint؛
- DOI و citation؛
- press kit شامل logo/screenshot/one-liner؛
- پست LinkedIn شخصی و پروژه؛
- معرفی McNeel Forum؛
- ایمیل هدفمند برای استادان/آزمایشگاه‌ها؛
- roadmap عمومی پس از 1.0.

### 37.3 hero demo

دموی اصلی باید:

1. scene واقعی و نسبتاً بزرگ را نشان دهد؛
2. تغییر پارامتریک massing/shade را نمایش دهد؛
3. update heatmap را بدون cut فریبنده نشان دهد؛
4. انتخاب sensor ضعیف را نشان دهد؛
5. occluder اصلی را highlight کند؛
6. timeline و درصد سهم مانع را نمایش دهد؛
7. CPU/Intel/NVIDIA یا حداقل backend portability را نشان دهد؛
8. report/benchmark link بدهد.

ویدئو نباید فقط animation از پیش renderشده باشد. تنظیمات و hardware در caption یا لینک ثبت می‌شوند.

### 37.4 پیام launch

پیام اصلی:

> XVARNA فقط نشان نمی‌دهد طرح چه نتیجه‌ای دارد؛ نشان می‌دهد چرا.

ادعاهای performance فقط از benchmark release برداشته می‌شوند. عبارت‌هایی مانند «سریع‌ترین جهان» بدون دامنه و مدرک ممنوع‌اند.

---

## 38. نقشه اجرا و زمان‌بندی کلان

زمان‌بندی برای یک توسعه‌دهنده اصلی متعهد با کمک ابزارهای هوش مصنوعی، reviewerهای تخصصی موردی و beta testerها نوشته شده است. تخمین‌ها پس از baseline velocity بازبینی می‌شوند.

### 38.1 بازه کلان

- تمام‌وقت منظم: حدود 9 تا 12 ماه برای RC، در صورت کنترل دامنه؛
- پاره‌وقت: حدود 14 تا 20 ماه؛
- اضافه‌کردن backend تخصصی چندگانه، Mac Tier A یا daylight path tracer کامل می‌تواند زمان را افزایش دهد.

تاریخ launch قبل از گذر از Gate 3 عمومی نمی‌شود.

### 38.2 فازها

| فاز | مدت هدف | خروجی اصلی |
|---|---:|---|
| 0. Charter & Setup | 2 هفته | spec freeze اولیه، repo، toolchain، CI |
| 1. Reference Geometry | 4 هفته | types، ingest، brute-force/reference intersection |
| 2. CPU Core | 6–8 هفته | BVH، query batch، cache، CLI، analytic corpus |
| 3. Domain Metrics I | 5–7 هفته | ZURVAN، Sun Hours، Shadow، SVF، attribution |
| 4. Grasshopper Foundation | 5–7 هفته | FFI، async components، scene، visualization |
| 5. Portable GPU | 6–9 هفته | wgpu traversal، parity، device/fallback |
| 6. Domain Metrics II | 6–8 هفته | radiation، sight، privacy، study/report |
| 7. Validated Daylight | 5–7 هفته | Radiance bridge، annual metric، comparison report |
| 8. Productization | 5–7 هفته | installer، docs، examples، Web viewer |
| 9. Closed Alpha/Beta | 6–10 هفته | real projects، fixes، performance/UX freeze |
| 10. Research & Release | 4–6 هفته همپوشان | benchmark paper، preprint، DOI، launch |

بعضی فازها همپوشانی محدود دارند؛ اما correctness هسته، FFI و مدل داده در critical path هستند.

### 38.3 critical path

```text
Spec/Toolchain
  → Canonical Types + Reference Intersection
  → Scene/BVH + Query Contract
  → CPU Correctness
  → FFI + GH Async Scene
  → Metric Semantics + Attribution
  → GPU Parity
  → Validation/Benchmark
  → Closed Beta
  → Release Candidate
  → Public 1.0
```

Web viewer، specialized backend و بعضی exportها نباید critical path correctness را مختل کنند.

---

## 39. Work Breakdown Structure

### WP-00 — Project Bootstrap

**خروجی‌ها:**

- Git repository؛
- README اولیه و link این spec؛
- Rust workspace؛
- `rust-toolchain.toml`؛
- .NET solution و target تصمیم‌گیری‌شده؛
- CI پایه؛
- license/NOTICE؛
- issue/RFC/ADR templates؛
- formatting/lint policy؛
- build script واحد.

**Exit:** clean clone روی Windows reference build/test شود.

### WP-01 — Canonical Math and Types

- vectors/matrices/AABB یا انتخاب library با audit؛
- ID types؛
- units/coordinate metadata؛
- error/result types؛
- config/version types؛
- serialization contracts؛
- deterministic hashing.

**Exit:** roundtrip/property tests و هیچ وابستگی Rhino.

### WP-02 — Geometry Ingest and Reference Kernel

- mesh validation؛
- canonicalization؛
- brute-force ray-triangle f64؛
- test scene generator؛
- CLI import اولیه؛
- analytic corpus v1.

**Exit:** reference result قابل اعتماد برای differential testing.

### WP-03 — BVH and CPU Production Backend

- BLAS/TLAS؛
- SAH baseline؛
- batch query؛
- parallel build/traversal؛
- SIMD experiments؛
- instance؛
- layer/category filter؛
- refit/rebuild؛
- profiling.

**Exit:** correctness gate و baseline performance report.

### WP-04 — Cache and Job Runtime

- content hash؛
- memory/disk cache؛
- scheduler؛
- async job؛
- cancel/progress؛
- result envelope؛
- diagnostics.

**Exit:** repeated interactive scenario بدون leak و با cache hit اثبات‌شده.

### WP-05 — ZURVAN/HVARE I

- EPW parser/validation؛
- sun position/vector؛
- period/filter/weights؛
- Direct Sun Hours؛
- Shadow Hours/Mask؛
- SVF؛
- object/time attribution.

**Exit:** analytic و reference tests برای تمام metricهای فاز.

### WP-06 — FFI and .NET Wrapper

- C header؛
- generated/manual safe .NET wrapper؛
- handle lifetime؛
- buffer marshalling؛
- async callback/poll؛
- error mapping؛
- ABI tests؛
- native library loader.

**Exit:** .NET console end-to-end بدون Rhino.

### WP-07 — Grasshopper Core UX

- component framework؛
- XV Info/Backend/Quality؛
- Scene/Sensor/Category؛
- Sun Hours/Sky View؛
- async state؛
- preview/legend/selection؛
- settings/cache UI؛
- packaging smoke.

**Exit:** Internal Alpha روی فایل نمونه واقعی.

### WP-08 — Portable GPU

- adapter enumeration؛
- buffers/layout؛
- BVH upload؛
- WGSL traversal؛
- chunking/readback؛
- device loss/fallback؛
- CPU/GPU differential؛
- profiling Intel/NVIDIA؛
- shader docs/tests.

**Exit:** parity gate و speed/memory report.

**وضعیت 0.11.0:** traversal، discovery، upload، chunking، readback، precision/memory report، CPU fallback و NVIDIA/Intel real-device parity علاوه بر raw queries برای سه domain کامل DAENA تحویل شده‌اند. موارد باز exit کامل: benchmark روی workloadهای A/B/C، fault-injected device loss، shader CI matrix، async/persistent buffer pipeline و تصمیم نهایی two-level BLAS/TLAS در برابر snapshot instance-expanded.

**وضعیت 0.12.0:** persistent/double-slot pipeline و corpus قابل‌تکرار cold/warm با min/p50/p95/mean، CPU parity و telemetry حافظه تحویل شده‌اند. callback و state machine خرابی device به‌صورت unit/ABI/API پوشش دارند. موارد باز exit کامل: اجرای منتشرشده corpus روی sceneهای واقعی A/B/C، fault injection واقعی در سطح driver، shader CI چندسیستمی و تصمیم نهایی two-level BLAS/TLAS.

**وضعیت 0.13.0:** تصمیم two-level نهایی و پیاده‌سازی شد؛ تست multi-chunk shader با CPU parity و exact attribution وارد suite شده است. انتشار benchmark A/B/C روی Intel/NVIDIA/AMD و driver-level fault injection هنوز شرط ادعای «برتری عملکرد عمومی» است، نه شرط صحت feature 0.13.

### WP-09 — HVARE II and Sight

- sky models؛
- direct/diffuse radiation؛
- Solar Access/Envelope؛
- Target/Weighted View؛
- isovist؛
- intervisibility/privacy؛
- visibility graph؛
- advanced attribution.

**Exit:** component + metric Definition of Done.

**وضعیت 0.11.0:** سه قابلیت advanced Sight شامل Target/Weighted/Green View، Corridor و Observer Path اکنون backend-neutral، GPU-capable و از CPU تا Rhino/CLI قابل parity validation هستند. Solar Access/Envelope و daylightهای باقی‌مانده مستقل از این تحویل‌اند.

### WP-10 — Study, Optimization Hooks and Report

- variant schema؛
- batch/resume؛
- compare/delta؛
- objective؛
- Pareto؛
- sensitivity validation؛
- HTML report؛
- CSV/JSON/Parquet؛
- glTF visualization.

**Exit:** multi-option case study کامل.

### WP-11 — Radiance Validation Bridge

- installation discovery/config؛
- material mapping؛
- scene export؛
- job runner؛
- point-in-time؛
- annual matrices/metrics؛
- comparison report؛
- reference dataset.

**Exit:** reproducible Fast vs Validated report.

### WP-12 — Web and CLI Productization

- CLI stable commands؛
- schema reader/writer؛
- web viewer؛
- WASM/shared types؛
- WebGPU compute experimental در صورت امکان؛
- artifact loading؛
- shareable static report.

**Exit:** browser demo بدون Rhino برای artifact نمونه.

### WP-13 — Quality, Docs and Packaging

- full docs؛
- tutorials؛
- examples؛
- installer/Yak؛
- signing/checksum/SBOM؛
- troubleshooting؛
- clean-machine tests؛
- localization infrastructure.

**Exit:** Closed Beta package.

### WP-14 — Research and Launch

- benchmark freeze؛
- representative architectural corpus runs؛
- validation report؛
- user study؛
- preprint؛
- DOI؛
- case studies؛
- hero video؛
- release 1.0.

**Exit:** تمام Gateهای بخش 40.

---

## 40. Release Gates

### Gate 0 — Charter Ready

- این سند review و baseline شده؛
- scope/non-goal روشن؛
- نام provisional؛
- toolchain plan؛
- backlog 8 هفته اول؛
- risk register فعال.

### Gate 1 — Reference Correctness

- canonical types؛
- f64 reference intersection؛
- analytic corpus؛
- import/validation؛
- zero known P0 correctness issue.

### Gate 2 — CPU Engine

- BLAS/TLAS/instances؛
- batch query؛
- cache/update؛
- CLI؛
- benchmark baseline؛
- stress/leak test؛
- docs معماری.

### Gate 3 — End-to-End Internal Alpha

- FFI stable draft؛
- GH Scene/Sensor/Sun Hours/Sky View؛
- async/cancel/progress؛
- attribution/highlight؛
- package داخلی؛
- حداقل یک case study واقعی.

### Gate 4 — GPU Parity

- portable GPU روی NVIDIA و Intel؛
- differential target پاس؛
- fallback/device loss؛
- memory budget؛
- performance gain یا دلیل حفظ backend؛
- shader test/docs.

**شواهد 0.11.0:** parity واقعی NVIDIA/Intel برای raw query و سه domain DAENA، shader test، memory/precision metadata، domain provenance و fallback قراردادی موجود است. Gate تا اجرای corpus performance، device-loss fault injection و CI چند-adapter در وضعیت «در حال تکمیل» باقی می‌ماند؛ smoke کوچک به‌تنهایی speedup claim نیست.

**شواهد 0.12.0:** benchmark رسمی اکنون cold را از warm جدا می‌کند، iterationهای warm را با min/p50/p95/mean ثبت می‌کند و invariantهای arena generation/reuse/device health را fail-closed می‌سنجد. هیچ ادعای speedup عمومی فقط از `open_quad` استخراج نمی‌شود؛ انتشار نتیجه performance نیازمند evidence فایل‌دار روی corpus A/B/C و چند adapter است.

### Gate 5 — Feature Complete

- تمام P0 و P1 تأییدشده نسخه 1.0؛
- component catalog؛
- Sight/Study/Report؛
- validated daylight workflow؛
- schema freeze candidate؛
- API freeze candidate.

### Gate 6 — Closed Beta Quality

- حداقل 20 tester؛
- حداقل 10 پروژه واقعی؛
- crash-free target؛
- install target؛
- zero S0/S1 شناخته‌شده؛
- docs/tutorial feedback؛
- benchmark/validation draft public-ready.

### Gate 7 — Release Candidate

- code/feature/API freeze؛
- all tests؛
- clean install matrix؛
- SBOM/license audit؛
- signed/checksummed artifacts؛
- final benchmark/validation؛
- security review؛
- release notes/migration؛
- launch assets.

### Gate 8 — Public 1.0

- RC حداقل دوره soak تعریف‌شده بدون blocker؛
- DOI/citation؛
- website/docs؛
- packages downloadable؛
- issue/support channels؛
- rollback/hotfix plan؛
- تصمیم go/no-go ثبت‌شده.

---

## 41. Definition of Done

یک feature فقط وقتی Done است که تمام موارد مرتبط زیر انجام شده باشند:

1. requirement ID و رفتار تعریف‌شده؛
2. design/ADR برای تصمیم غیرساده؛
3. implementation production؛
4. error handling و cancellation؛
5. unit/property/integration tests؛
6. differential/validation در صورت metric/backend؛
7. benchmark در صورت hot path؛
8. documentation و help؛
9. example قابل اجرا؛
10. accessibility/UX review برای component؛
11. logging/diagnostics؛
12. security/input bounds؛
13. license/provenance dependency؛
14. changelog؛
15. review؛
16. CI سبز؛
17. هیچ P0/P1 bug باز مربوط به feature.

عبارت «کدش کار می‌کند» معادل Done نیست.

### 41.1 Definition of Done برای metric

علاوه بر موارد بالا:

- تعریف ریاضی/قراردادی؛
- واحد؛
- assumptions؛
- valid input domain؛
- quality tiers؛
- uncertainty؛
- analytic/reference cases؛
- output interpretation؛
- limitations؛
- citation method.

### 41.2 Definition of Done برای backend

- capability matrix؛
- parity tests؛
- error/fallback؛
- memory budget؛
- device matrix؛
- performance report؛
- deterministic mode؛
- packaging؛
- failure recovery.

---

## 42. منابع انسانی، ابزار و بودجه

### 42.1 نقش‌های لازم

یک نفر می‌تواند مالک اصلی باشد، اما برای کیفیت 1.0 این نقش‌ها باید پوشش داده شوند:

- Product/Research Lead؛
- Rust/Core Engineer؛
- GPU/Shader Engineer؛
- Rhino/.NET Engineer؛
- Building Performance/Daylight Reviewer؛
- UX/Visual Design؛
- QA/Benchmark؛
- Technical Writer؛
- Beta Community Manager.

یک نفر ممکن است چند نقش داشته باشد، اما review daylight، GPU و UX بهتر است حداقل به‌صورت مشاوره بیرونی انجام شود.

### 42.2 نیازهای سخت‌افزاری تست

- دستگاه مرجع فعلی NVIDIA + Intel؛
- یک GPU AMD؛
- یک Mac Apple Silicon؛
- یک سیستم CPU-only/low-end؛
- در صورت امکان یک workstation با VRAM بالا؛
- CI runner یا دستگاه شبانه GPU.

خرید یا دسترسی به این سخت‌افزار باید پیش از Gate 4/6 برنامه‌ریزی شود.

### 42.3 هزینه‌های احتمالی

- domain/branding/trademark search؛
- code signing certificate؛
- سخت‌افزار تست؛
- hosting documentation/release assets؛
- conference/publication؛
- reviewer/consultant؛
- ویدئو/طراحی بصری؛
- DOI معمولاً از مسیرهای دانشگاهی/Zenodo کم‌هزینه است؛ policy هنگام انتشار بررسی می‌شود.

بودجه دقیق پس از WP-00 در فایل project management جدا ثبت می‌شود.

---

## 43. Risk Register

| ID | ریسک | احتمال | اثر | کاهش ریسک | trigger/contingency |
|---|---|---:|---:|---|---|
| `R-001` | scope بیش از ظرفیت | زیاد | بحرانی | P0/P1/P2، gate، RFC | تأخیر دو milestone؛ freeze قابلیت و انتقال P2 |
| `R-002` | performance GPU کمتر از انتظار | متوسط | زیاد | CPU قوی، چند layout، benchmark زود | portable backend فقط parity؛ بررسی specialized یا تمرکز incremental |
| `R-003` | اختلاف CPU/GPU edge case | زیاد | زیاد | reference f64، corpus، tolerance policy | metric release block تا classification |
| `R-004` | wgpu/hardware RT ناپایدار | متوسط | زیاد | compute traversal مستقل | pin version/fallback CPU |
| `R-005` | VRAM کم دستگاه‌های دانشجویی | زیاد | زیاد | chunking، budget، CPU fallback | کاهش diagnostic payload، out-of-core |
| `R-006` | crash Rhino از FFI/native | متوسط | بحرانی | ABI tests، panic boundary، soak | disable backend problematic و hotfix |
| `R-007` | Radiance mapping پیچیده | زیاد | زیاد | scope materials روشن، reference cases | validated subset شفاف؛ انتقال advanced material به 1.x |
| `R-008` | ادعای علمی بیش از شواهد | متوسط | بحرانی | claim registry، review، benchmark public | اصلاح messaging و limitation |
| `R-009` | نبود دسترسی AMD/Mac | متوسط | زیاد | تهیه tester/hardware زود | کاهش support tier با اعلام صریح |
| `R-010` | تداخل نام/trademark | متوسط | زیاد | search رسمی قبل branding | rename پیش از public beta |
| `R-011` | dependency/license ناسازگار | متوسط | زیاد | audit از ابتدا | جایگزینی dependency یا جداسازی optional |
| `R-012` | single-maintainer burnout | زیاد | زیاد | milestone کوچک، automation، community | کاهش scope P2 و جذب maintainer |
| `R-013` | benchmark overfitting | متوسط | زیاد | hidden/holdout scenes، external review | اضافه‌کردن dataset مستقل |
| `R-014` | feedback دیرهنگام UX | متوسط | زیاد | Closed Alpha زود پس از Gate 3 | redesign قبل API/component freeze |
| `R-015` | plugin dependency conflict | متوسط | زیاد | private native deps، loader isolation | diagnostic conflict و repack |
| `R-016` | result misuse برای compliance | متوسط | زیاد | quality labels، report limitations | watermark/disable claim در Fast Path |
| `R-017` | file/parser security bug | کم/متوسط | زیاد | fuzz، limits، security policy | patch/advisory |
| `R-018` | schema/API freeze زودهنگام | متوسط | متوسط | beta feedback قبل freeze | migration/compat layer |
| `R-019` | dataset حقوقی نامناسب | متوسط | زیاد | provenance/license gate | حذف/جایگزینی dataset |
| `R-020` | کیفیت documentation ناکافی | متوسط | زیاد | docs DoD و user testing | توقف launch تا onboarding pass |

Risk register در پایان هر milestone بازبینی می‌شود.

---

## 44. تصمیم‌های قفل‌شده و پرسش‌های باز

### 44.1 تصمیم‌های قفل‌شده

| ID | تصمیم |
|---|---|
| `DEC-001` | اولین انتشار عمومی، 1.0 پرچم‌دار است؛ demo عمومی کم‌دامنه نداریم. |
| `DEC-002` | هسته اصلی Rust است. |
| `DEC-003` | Grasshopper adapter با C#/.NET و FFI باریک ساخته می‌شود. |
| `DEC-004` | CPU backend اجباری و GPU اختیاری/fallback‌پذیر است. |
| `DEC-005` | fast path از validated path صریحاً جدا می‌شود. |
| `DEC-006` | attribution قابلیت مرکزی محصول است. |
| `DEC-007` | واحد canonical فاصله meter است. |
| `DEC-008` | scene snapshot immutable و revisioned است. |
| `DEC-009` | cloud برای عملکرد اصلی اجباری نیست. |
| `DEC-010` | benchmark و validation عمومی هستند. |
| `DEC-011` | کل HVAC/CFD/CAD kernel خارج از 1.0 است. |
| `DEC-012` | نام provisional محصول XVARNA است. |
| `DEC-013` | license پیشنهادی Apache-2.0 است. |
| `DEC-014` | connector رسمی XVARNA، Rhino 8 را با `net8.0` و Rhino 9 را با `net10.0` به‌صورت AnyCPU پشتیبانی می‌کند. |
| `DEC-015` | .NET Framework و `net7.0` target رسمی محصول جدید نیستند. |

### 44.2 پرسش‌های باز با موعد تصمیم

| ID | پرسش | موعد |
|---|---|---|
| `OPEN-001` | availability/trademark نهایی XVARNA | قبل از public branding |
| `OPEN-002` | نسخه pin دقیق SDKهای Rhino 8 و Rhino 9 و cadence ارتقا | WP-00؛ framework حل شد |
| `OPEN-003` | انتخاب math/hash/serialization dependencies | WP-01 |
| `OPEN-004` | BVH layout نهایی CPU/GPU | پایان WP-03/WP-08 |
| `OPEN-005` | OptiX/CUDA در 1.0 یا 1.x | پس از portable benchmark |
| `OPEN-006` | Mac Tier A یا B | قبل Closed Beta |
| `OPEN-007` | WebGPU compute در 1.0 stable یا experimental | پایان WP-12 |
| `OPEN-008` | فرمت binary result دقیق | WP-04/WP-10 |
| `OPEN-009` | DCO یا CLA | قبل external contribution |
| `OPEN-010` | حدود Fast Daylight material/multi-bounce | design WP-11 |
| `OPEN-011` | thresholds رسمی KPI بعد dataset freeze | Gate 2 |
| `OPEN-012` | code signing و distribution channels | قبل Gate 6 |

---

## 45. Backlog اجرایی شروع پروژه

### Sprint 0 — دو هفته اول

1. تأیید نام provisional و ایجاد repository؛
2. نصب toolchain ناقص؛
3. انتخاب target .NET/Rhino template؛
4. ایجاد Cargo workspace و .NET solution؛
5. license، README، CONTRIBUTING، RFC/ADR؛
6. CI build/test؛
7. crate `xvarna-types`؛
8. crate `xvarna-geometry`؛
9. CLI skeleton؛
10. procedural test scene generator؛
11. benchmark harness skeleton؛
12. تصمیم hash/math crates؛
13. issueهای WP-01/WP-02؛
14. ثبت baseline ابزار و دستگاه.

### Sprint 1 — Reference Kernel

1. MeshResource validation؛
2. units/origin normalization؛
3. deterministic hash؛
4. reference ray-triangle f64؛
5. brute-force batch؛
6. Hit Record؛
7. analytic plane/box/edge tests؛
8. JSON manifest اولیه؛
9. CLI `inspect` و `trace`؛
10. correctness report v0.

### Sprint 2 — First BVH

1. AABB/intersection؛
2. median split baseline؛
3. binned SAH؛
4. flatten layout baseline؛
5. closest/any hit؛
6. differential test؛
7. Criterion یا harness equivalent؛
8. memory stats؛
9. OBJ/PLY fixture منتخب؛
10. ADR انتخاب layout موقت.

### اولین demo داخلی مجاز

CLI که یک scene procedural را load می‌کند، یک sensor grid و مجموعه ray را اجرا می‌کند و نتیجه CPU reference/BVH را همراه timing و mismatch report می‌دهد. این demo منتشر یا بازاریابی نمی‌شود؛ ابزار verification است.

---

## 46. مدیریت پروژه و ریتم کار

### 46.1 cadence

- sprint دو هفته‌ای؛
- برنامه sprint با issueهای محدود؛
- review هفتگی risk/performance؛
- demo داخلی پایان sprint؛
- release gate review در milestone؛
- changelog و decision log پیوسته.

### 46.2 issue template feature

هر feature issue:

- requirement ID؛
- user value؛
- scope/non-scope؛
- design notes؛
- dependencies؛
- acceptance tests؛
- performance/accuracy budget؛
- docs/example؛
- risks؛
- Definition of Done checklist.

### 46.3 claim registry

فایلی جدا باید تمام ادعاهای عمومی را نگه دارد:

- claim؛
- دامنه؛
- evidence artifact؛
- نسخه/hardware؛
- owner؛
- تاریخ انقضا/بازبینی.

هر claim launch بدون evidence حذف می‌شود.

### 46.4 debt policy

- `unsafe`, shader workaround، skipped test و precision exception با issue و owner؛
- debt بحرانی قبل Gate بعدی حل می‌شود؛
- TODO بدون issue در hot path ممنوع؛
- benchmark hack وارد production path نمی‌شود؛
- refactor بزرگ پس از feature freeze فقط برای blocker.

---

## 47. معیار توقف یا تغییر مسیر

پروژه ambition بالایی دارد، اما باید evidence-driven بماند. pivot یا کاهش دامنه بررسی می‌شود اگر:

- پس از چند iteration، engine هیچ مزیت performance/workflow نسبت به baseline نشان ندهد؛
- GPU parity به‌طور پایدار غیرقابل دستیابی باشد؛
- integration Rhino ناپایدار و crash-prone بماند؛
- کاربران attribution را بی‌ارزش بدانند؛
- scope مانع رسیدن به محصول قابل انتشار شود؛
- نام یا license مانع حقوقی جدی ایجاد کند.

pivot به معنی شکست نیست. هسته می‌تواند روی بهترین ارزش اثبات‌شده، مثلاً attribution + incremental visibility، متمرکز شود. هر pivot با RFC و حفظ artifactهای پژوهشی انجام می‌شود.

---

## 48. واژه‌نامه

| اصطلاح | تعریف در این پروژه |
|---|---|
| ABI | قرارداد binary میان Rust و clientها |
| Attribution | نسبت‌دادن سهم نتیجه/انسداد به object، category یا time |
| BLAS | acceleration structure هندسه پایه |
| BVH | Bounding Volume Hierarchy |
| Cache Hit | استفاده معتبر از artifact با key یکسان |
| Counterfactual | محاسبه یا برآورد نتیجه در صورت حذف/تغییر عامل |
| Deterministic | ورودی/config/seed یکسان، نتیجه در tolerance قراردادی یکسان |
| Direct Sun Hours | مجموع مدت دریافت مستقیم خورشید |
| EPW | EnergyPlus Weather file |
| Fast Path | مدل تعاملی داخلی با محدودیت اعلام‌شده |
| FFI | Foreign Function Interface |
| Isovist | ناحیه/حجم قابل رؤیت از نقطه تحت قیود |
| Metric Version | نسخه semantic تعریف و الگوریتم metric |
| North Rotation | زاویه model north نسبت به true north |
| Occluder | مانع ray/visibility |
| P0/P1/P2 | اولویت release طبق بخش 0 |
| Provenance | زنجیره منشأ ورودی، تنظیمات، engine و artifact |
| Q0–Q4 | tier کیفیت محاسبه |
| Ray Batch | مجموعه queryهای ray با metadata مشترک |
| Reference Backend | implementation کندتر/دقیق‌تر برای verification |
| Refit | update bounds بدون rebuild کامل topology BVH |
| Scene Snapshot | نسخه immutable و hash‌شده scene |
| Sensor | نقطه/جهت/وزن نمونه‌برداری |
| Solid Angle | اندازه زاویه‌ای سه‌بعدی هدف/FOV |
| Stale Result | نتیجه متعلق به input/revision قدیمی |
| TLAS | acceleration structure instance/scene level |
| Validated Path | workflow مقایسه یا اجرا با engine مرجع |
| Visibility Graph | graph اتصال line-of-sight میان nodeها |
| WGSL | زبان shader در WebGPU/wgpu |

---

## 49. منابع پایه و شواهد اولیه

این فهرست نقطه شروع است و bibliography پژوهشی در طول پروژه گسترش می‌یابد.

### پلتفرم و SDK

- [Rhino Developer Documentation](https://developer.rhino3d.com/en/)
- [Rhino C++ SDK Overview](https://developer.rhino3d.com/en/guides/cpp/what-is-the-cpp-sdk/)
- [Grasshopper SDK Documentation](https://developer.rhino3d.com/api/grasshopper/html/723c01da-9986-4db2-8f53-6f3a7494df75.htm)

### نیاز و ابزارهای محیطی

- [Ladybug Tools](https://www.ladybug.tools/)
- [Ladybug Direct Sun Hours documentation](https://docs.ladybug.tools/ladybug-primer/components/3_analyzegeometry/direct_sun_hours)
- [Honeybee and its validated engines](https://www.ladybug.tools/honeybee.html)
- [Cyclops announcement and hardware discussion](https://discourse.mcneel.com/t/introducing-cyclops-real-time-raytracing-for-sustainable-design/202912)
- [NVIDIA/Foster + Partners Cyclops case study](https://www.nvidia.com/en-us/case-studies/foster-partners/)
- [depthmapX](https://github.com/SpaceGroupUCL/depthmapX)
- [ANSI/IES LM-83-23 — sDA and ASE](https://ies.org/standards/lighting-library/)

### زیرساخت فنی

- [wgpu](https://github.com/gfx-rs/wgpu)
- [wgpu experimental ray tracing specification](https://github.com/gfx-rs/wgpu/blob/trunk/docs/api-specs/ray_tracing.md)
- [Rust bvh crate](https://github.com/svenstaro/bvh)
- [Manifold geometry library](https://github.com/elalish/manifold)
- [Truck CAD kernel](https://github.com/ricosjp/truck)
- [Radiance](https://www.radiance-online.org/)
- [glTF specification](https://www.khronos.org/gltf/)

### نام و فرهنگ

- [Encyclopaedia Iranica — Farr/Xvarənah](https://www.iranicaonline.org/articles/farrah/)
- [Encyclopaedia Iranica — Zurvan](https://www.iranicaonline.org/articles/zurvan-deity/)
- [Encyclopaedia Iranica — Rashnu](https://www.iranicaonline.org/articles/rasn-deity/)
- [Avestan Dictionary — Hvar/Hvare](https://www.avesta.org/avdict/)

### مجوز و انتشار

- [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)
- [Citation File Format](https://citation-file-format.github.io/)
- [Zenodo](https://zenodo.org/)

---

## Appendix A — نمونه config هنجاری

نمونه صرفاً برای تثبیت شکل مفهومی است و schema نهایی در WP-01/WP-04 تعیین می‌شود.

```json
{
  "schema": "dev.xvarna.job/0.1",
  "metric": {
    "id": "dev.xvarna.direct-sun-hours",
    "version": "1.0.0"
  },
  "scene": {
    "id": "example-site",
    "revision": 12,
    "units": "m",
    "trueNorthDegrees": 17.5
  },
  "quality": {
    "tier": "Q2",
    "deterministic": true,
    "seed": 424242
  },
  "backend": {
    "preference": "auto",
    "allowCpuFallback": true,
    "memoryBudgetMiB": 3072
  },
  "time": {
    "timezone": "Asia/Tehran",
    "schedule": "occupied",
    "minimumSunAltitudeDegrees": 0.5
  },
  "geometry": {
    "absoluteToleranceMeters": 0.001,
    "relativeTolerance": 1e-7,
    "originRebase": "auto"
  },
  "result": {
    "includeTimeline": true,
    "includeTopOccluders": 10,
    "includeTriangleIds": false
  }
}
```

---

## Appendix B — ماتریس قابلیت backend

| قابلیت | CPU | Portable GPU | NVIDIA Specialized | Radiance |
|---|---:|---:|---:|---:|
| AnyHit visibility | بله | بله | هدف | غیرمستقیم |
| ClosestHit attribution | بله | بله | هدف | محدود به workflow |
| Instances | بله | بله | هدف | export-based |
| Dynamic refit | بله | هدف | هدف | خیر |
| f64 reference | بله | معمولاً خیر | خیر | مستقل |
| Fast Sun/Sky | بله | بله | هدف | کندتر/reference |
| Annual validated daylight | bridge | bridge | bridge | بله |
| CPU-only system | بله | خیر | خیر | بله |
| Intel/AMD/NVIDIA | CPU vendor-neutral | هدف | NVIDIA فقط | CPU |
| Browser | WASM fallback احتمالی | WebGPU experimental | خیر | خیر |

---

## Appendix C — Release 1.0 Master Checklist

### Product

- [ ] تمام P0ها؛
- [ ] تمام P1های تأییدشده؛
- [ ] workflowهای بخش 9؛
- [ ] component catalog پایدار؛
- [ ] UX Basic/Expert؛
- [ ] attribution end-to-end؛
- [ ] report و export.

### Engineering

- [ ] CPU backend؛
- [ ] portable GPU؛
- [ ] cache/update؛
- [ ] FFI/API freeze؛
- [ ] schema freeze؛
- [ ] async/cancel/progress؛
- [ ] security and supply chain؛
- [ ] package clean install.

### Correctness and Research

- [ ] analytic corpus؛
- [ ] CPU/GPU differential؛
- [ ] Radiance validation؛
- [ ] absolute performance corpus؛
- [ ] uncertainty/limitations؛
- [ ] raw data/scripts؛
- [ ] user study یا حداقل usability study تعریف‌شده؛
- [ ] preprint/technical report.

### Documentation and Community

- [ ] همه componentها مستند؛
- [ ] 12 tutorial؛
- [ ] examples تست‌شده؛
- [ ] contributing/security/governance؛
- [ ] citation/DOI؛
- [ ] support channels؛
- [ ] roadmap 1.x.

### Launch

- [ ] نام و trademark check؛
- [ ] logo/visual identity؛
- [ ] landing page؛
- [ ] hero demo؛
- [ ] case studies؛
- [ ] benchmark/validation pages؛
- [ ] signed/checksummed artifacts؛
- [ ] announcement package؛
- [ ] hotfix/rollback plan.

---

## Appendix D — Change Log این سند

### 0.1.0 — 2026-08-30

- ایجاد سند مادر؛
- تثبیت نام provisional XVARNA؛
- تعریف دامنه flagship 1.0؛
- ثبت معماری Rust/C#/CPU/GPU/Radiance؛
- تعریف component catalog، data model، quality tiers و metricها؛
- تعریف validation، benchmark، release gates، roadmap و risk register.

### 0.2.0 — 2026-08-30

- تصویب baseline با فرمان شروع اجرای پروژه؛
- افزودن پشتیبانی رسمی هم‌زمان Rhino 8 و Rhino 9؛
- تثبیت `net8.0` برای Rhino 8.20+ و `net10.0` برای Rhino 9؛
- حذف `net7.0` و `net48` از target رسمی محصول جدید؛
- تثبیت AnyCPU برای connector و ABI مستقل native core.

### 0.3.0 — 2026-08-31

- تحویل `ZAMYAD Scene Engine` به‌عنوان زیربنای فضایی واقعی محصول؛
- افزودن deduplication محتوایی مش‌ها، BLAS/TLAS قطعی با binned SAH، instancing و origin rebasing؛
- افزودن closest-hit و any-hit دسته‌ای موازی با attribution کامل object/instance/mesh/triangle؛
- انتشار C ABI مبتنی بر handle با lifecycle امن، panic containment و layoutهای ثابت؛
- انتشار API مدیریت‌شده برای `net8.0` و `net10.0` و کامپوننت‌های `XV Scene` و `XV Ray Query`؛
- افزودن CLI benchmark و differential validation در برابر reference kernel؛
- ارتقای ABI افزایشی از `0.2.0` به `0.3.0`.

### 0.4.0 — 2026-08-31

- تثبیت راهبرد نام‌گذاری: نام‌های ایرانی باستان برای موتورهای اصلی و نام‌های عملکردی برای کامپوننت‌ها؛
- تحویل `ZURVAN` fixed-offset period/schedule با timestamp میانی، duration و weight صریح؛
- تحویل `HVARE` Sun Vectors بر پایه پیاده‌سازی مستقل Reda–Andreas SPA و golden case منتشرشده NREL؛
- تحویل Direct Sun Hours، Shadow Hours، Solar Access، timeline کامل و dominant-occluder attribution؛
- افزودن C ABI، API مدیریت‌شده، CLI و کامپوننت‌های `XV Period`، `XV Sun Vectors` و `XV Sun Hours` برای Rhino 8/9؛
- ارتقای ABI افزایشی از `0.3.0` به `0.4.0`.

### 0.5.0 — 2026-08-31

- تثبیت نام `ASMAN` برای موتور تحلیل آسمان؛ نام برگرفته از مفهوم و شخصیت‌بخشی آسمان در سنت ایرانی است؛
- تحویل hemisphere sampling قطعی Fibonacci با وزن‌دهی solid-angle برابر، seed پایدار و basis سازگار با normal سنسور؛
- تحویل Sky View Factor لامبرتی، visible-hemisphere fraction، solid angle، convergence diagnostic و dominant-occluder attribution؛
- تحویل Shadow Mask کامل با direction و first-hit object/instance/mesh/triangle/distance برای هر نمونه؛
- افزودن job handle، progress atomics و cancellation واقعی بین chunkهای ray در Rust و C ABI؛
- افزودن API هم‌زمان/قابل‌لغو/async برای .NET 8 و .NET 10؛
- افزودن `XV Sky View` task-based با heatmap و `XV Shadow Mask` با preview سه‌بعدی برای Rhino 8/9؛
- افزودن CLI مستقل `sky-view` با JSON و CSV کامل Shadow Mask؛
- ارتقای ABI افزایشی از `0.4.0` به `0.5.0`.

### 0.6.0 — 2026-08-31

- تثبیت نام `MEHR` برای موتور تابش سالانه که ZURVAN، ASMAN و ZAMYAD را در یک pipeline علمی ترکیب می‌کند؛
- تحویل parser سخت‌گیر EPW با پشتیبانی hourly/subhourly، تبدیل end-of-interval local standard time به midpoint UTC و diagnostics مقادیر گمشده؛
- تحویل مدل Perez 1990 سه‌مولفه‌ای مطابق فرمول‌بندی EnergyPlus برای سطح مایل؛
- تحویل direct/circumsolar closest-hit، diffuse dome با 144 ray، horizon با 24 ray و ground-reflected isotropic؛
- تحویل timeline کامل انرژی و attribution شامل object/instance/mesh/triangle/distance، dominant blocker و lost Wh/m²؛
- افزودن cancellation/progress، C ABI، API هم‌زمان/async برای .NET 8/10، `XV Irradiance` برای Rhino 8/9 و CLI/JSON/CSV؛
- افزودن قرارداد علمی، non-claimها و تست‌های EPW/Perez/ground/blocker/ABI/runtime؛
- ارتقای ABI افزایشی از `0.5.0` به `0.6.0`.

### 0.7.0 — 2026-08-31

- تحویل pipeline کاربردی کامل `Geometry → Sensor Grid → MEHR Irradiance → Solar Map → Eligible Regions → PV Proxy`؛
- افزودن longest-edge subdivision قطعی در Rust با تضمین maximum edge، حفظ مساحت، offset، شناسه پایدار، provenance مثلث مبدأ، depth و BLAKE3 identity؛
- افزودن hard cell ceiling و failure صریح برای جلوگیری از انفجار حافظه در resolutionهای نامناسب؛
- افزودن mean/P10/P50/P90 وزن‌دار بر اساس مساحت واقعی و heatmap با domain کامل و palette یکنواخت Viridis؛
- افزودن threshold mask و region extraction قطعی با exact-edge connectivity و ثبت محدودیت fragmentation در T-junctionها؛
- تحویل `SOL-013 PV Potential Proxy` با incident energy، eligible area، module efficiency، coverage، combined loss، capacity kWp، proxy yield kWh و specific yield؛
- تثبیت non-claim: خروجی PV صرفاً screening proxy است و bankable yield یا جایگزین SAM/PVWatts/layout engineering نیست؛
- افزودن C ABI دو-مرحله‌ای، API عمومی .NET 8/10، CLI `surface-grid` و کامپوننت‌های `XV Sensor Grid` و `XV Solar Potential` برای Rhino 8/9؛
- افزودن تست‌های conservation/resolution/determinism/resource/unit-scale/weighted-region/PV/ABI و ارتقای ABI افزایشی از `0.6.0` به `0.7.0`.

### 0.8.0 — 2026-08-31

- تثبیت نام `DAENA` برای موتور Spatial Visibility با معنای تاریخی vision/insight و کاربرد مستقیم در Sight/Privacy؛
- تحویل 2D Isovist قطعی روی plane دلخواه با ordered polygon/radials، area، perimeter، centroid distance، radial min/mean/max/σ/skew، compactness، convergence delta، finite clipping و dominant first blocker؛
- تحویل directed Intervisibility Matrix کامل با stateهای visible/blocked/out-of-range/coincident و object/instance/mesh/triangle attribution؛
- تحویل privacy screening proxy شفاف با observer weight، target sensitivity، facing cosine/exponent و distance decay و combined-risk complement product؛
- تحویل exact all-pairs Visibility Graph با blocked-pair map، degree/degree centrality، deterministic connected components، harmonic closeness و Brandes betweenness؛
- افزودن hard capهای 4,096 graph node و 16,000,000 matrix/result entry و failure صریح قبل از مصرف کنترل‌نشده حافظه؛
- افزودن crate مستقل `xvarna-daena`، C ABI ثابت افزایشی، API عمومی unit-aware برای .NET 8/10 و سه CLI مستقل با JSON 0.8 و CSV کامل؛
- افزودن `XV Isovist`، `XV Intervisibility` و `XV Visibility Graph` با Rhino geometry و Data Tree برای Rhino 8/.NET 8 و Rhino 9/.NET 10؛
- افزودن قرارداد علمی/فرمول/non-claim، ADR، component guideها و تست‌های analytic room/divider/category/privacy/graph/ABI/unit/runtime؛
- ارتقای ABI افزایشی از `0.7.0` به `0.8.0` و افزایش gate runtime به 15 کامپوننت.

### 0.9.0 — 2026-08-31

- تکمیل DAENA Target View / Weighted View / Green View بر پایه solid angle دقیق، partial-occlusion sampling قطعی، weighting شفاف، category masks، convergence و first-blocker attribution؛
- تحویل View Corridor با aperture دایره‌ای uniform-area، open solid angle، dominant/nearest conflict و provenance کامل؛
- تحویل Dynamic Observer Path با uniform distance sampling، tangent camera، trapezoidal aggregation و best/worst hotspots؛
- تثبیت نام `VAHMAN` برای XVARNA Study/Optimization و افزودن crate مستقل `xvarna-study`؛
- تحویل constraint-aware Pareto ranking، NSGA-II crowding، exact 2D hypervolume و descriptive Spearman ρ؛
- تحویل optimizer قطعی ask/tell با continuous/integer/categorical variables، stratified initialization، tournament، SBX، polynomial/discrete mutation و bounded historical archive؛
- افزودن C ABI ثابت، safe-handle API در .NET 8/10، پنج CLI و شش component جدید برای Rhino 8/9؛
- افزودن ADR، قرارداد علمی/non-claim، component guides، تست analytic/reproducibility/layout/unit/runtime و smoke gateهای کامل؛
- ارتقای ABI افزایشی از `0.8.0` به `0.9.0` و افزایش gate runtime به 21 کامپوننت.

### 0.10.0 — 2026-09-01

- تثبیت نام `VAYU` برای portable compute بر پایه معنای ایرانی باستانی wind/atmosphere/space؛
- افزودن crate مستقل `xvarna-vayu` با `wgpu 30.0.1`، WGSL closest/any-hit traversal و backendهای قابل‌حمل؛
- تحویل portable scene snapshot با canonical origin rebasing، اندازه‌گیری خطای f64→f32، conservative BVH bounds، hard triangle/depth limits و exact 64-bit attribution packing؛
- تحویل Auto/required-GPU/CPU sessions، adapter discovery، bounded dispatch chunking، timing تفکیکی upload/execute/readback، memory/precision provenance و creation/runtime fallback؛
- افزودن C ABI و safe-handle .NET 8/10 با adapter filter، `XV Devices`، `XV Backend` و ارتقای backend-aware `XV Ray Query` برای Rhino 8/9؛
- افزودن CLIهای `devices` و `backend-benchmark` با adapter filtering، parity state/identity/distance و JSON نسخه‌دار؛
- ثبت smoke differential واقعی روی NVIDIA RTX 5060 و Intel Raptor Lake/Vulkan با صفر state/identity mismatch و non-claim صریح برای performance روی scene کوچک؛
- ارتقای ABI افزایشی از `0.9.0` به `0.10.0` و افزایش gate runtime به 23 کامپوننت.

### 0.11.0 — 2026-09-01

- افزودن `RayQueryExecutor` به‌عنوان قرارداد backend-neutral در `xvarna-scene` و implementation مشترک CPU f64/VAYU؛
- مهاجرت کامل DAENA Target/Weighted/Green View، View Corridor و Dynamic Observer Path به domain batchهای مرتب و قابل chunk؛
- افزودن execution provenance تجمیعی شامل backend، adapter، fallback، batch/ray/dispatch، upload/execute/readback و maximum precision error؛
- افزودن C ABI سطح بالا و `xv_daena_execution_info` با layout ثابت و APIهای source-unit-aware در .NET 8/10؛
- ارتقای `XV Target View`، `XV Corridor` و `XV View Path` برای پذیرش هم‌زمان `XV Scene` و `XV Backend` بدون تغییر GUID/output index؛
- افزودن backend flags و fail-closed `--verify-cpu` برای هر سه CLI و ثبت parity واقعی چند-dispatch روی NVIDIA/Intel؛
- ارتقای ABI، managed assemblies، CLI schema و package version از `0.10.0` به `0.11.0` با حفظ gate runtime 23 کامپوننت.

### 0.12.0 — 2026-09-01

- بازطراحی hot path VAYU با arena دو-slot، buffer/bind-group reuse، host-vector reuse، queue writes و submission/readback جفتی؛
- افزودن device-loss و uncaptured-error callbacks، health state ماندگار، fallback سریع Auto و diagnostic دقیق؛
- افزودن telemetry تجمعی production به Rust و ABI افزایشی 368-byte، API مدیریت‌شده .NET 8/10 و کامپوننت جدید `XV Runtime` برای Rhino 8/9؛
- ارتقای reportهای batch و DAENA برای اثبات استفاده از persistent arena بدون تغییر layoutهای موجود؛
- ارتقای `backend-benchmark` به cold/warm benchmark با warm-up، iteration، min/p50/p95/mean، transfer breakdown و memory/device telemetry؛
- افزودن `eng/benchmark-vayu-throughput.ps1` برای evidence چنداندازه‌ای و invariantهای fail-closed؛
- ارتقای ABI، managed assemblies، CLI schema و package version از `0.11.0` به `0.12.0` و افزایش gate runtime به 24 کامپوننت.

### 0.13.0 — 2026-09-02

- تحویل scene delta transaction اتمیک، static/dynamic layers، snapshotهای immutable و revision قابل مشاهده؛
- تحویل reuse/refit/rebuild مستقل TLAS هر لایه با quality ratio و consecutive-refit rebuild policy و BLAS sharing/replacement؛
- جایگزینی snapshot تخت GPU با TLAS/BLAS instancing واقعی در WGSL و حفظ exact 64-bit attribution؛
- تحویل geometry chunking و streaming قطعی تحت بودجهٔ جداگانهٔ resident geometry و total VRAM همراه telemetry upload؛
- تحویل cache واقعی memory/disk با BLAKE3 key/checksum، schema، atomic writes، eviction، corruption recovery و کنترل Rust/C/.NET/CLI/Rhino؛
- تحویل scheduler مشترک bounded، async task execution، cancellation، progress phase/fraction، stale-result suppression و `XV Scheduler`؛
- اتصال lifecycle به `XV Scene` و revision invalidation به `XV Backend` و افزودن `XV Cache`؛
- ارتقای ABI/package/schema به `0.13.0` و gate runtime یکسان 26 کامپوننت برای Rhino 8/.NET 8 و Rhino 9/.NET 10.

### 0.14.0 — 2026-09-02

- تحویل 3D Isovist با نمونه‌برداری equal-solid-angle، حجم و radial-surface، visible solid angle، openness، convergence و ray field کامل؛
- عمومی‌سازی top-K attribution و exact category-mask breakdown همراه remove-one counterfactual با retrace واقعی و کشف blockerهای downstream؛
- تحویل Landmark Visibility بر پایه camera FOV، apparent solid angle کروی، partial disk sampling، وزن اهمیت و material transmission؛
- تحویل Material System immutable برای visible/direct-solar transmittance و reflectance، assignment با Object ID و conservation validation؛
- تحویل comparison هم‌تراز سناریوهای فضایی و delta attribution به صورت candidate-minus-baseline؛
- تحویل comparison سناریوهای خورشیدی با timeline کامل، received/lost hours، top-K/category attribution و baseline deltas؛
- تحویل Solar/Shading Envelope برای vertical-column early-massing با weighted quantile، clearance و controlling sensor/UTC؛
- افزودن highlight مستقیم occluderها در Rhino با exact Object ID و هفت کامپوننت جدید؛
- افزودن چهار CLI، C ABI ثابت، API source-unit-aware در .NET 8/10 و قرارداد علمی/non-claim؛
- ارتقای ABI/package/schema به `0.14.0` و gate runtime یکسان 33 کامپوننت برای Rhino 8/.NET 8 و Rhino 9/.NET 10.

**مرز ادعا:** Solar Envelope جایگزین ضوابط حقوقی، daylight/radiosity یا هندسهٔ آزاد سه‌بعدی نیست؛ radial surface در 3D Isovist نیز `Σ(ωr²)` است و مساحت پوستهٔ triangulated محسوب نمی‌شود. ادعای برتری عمومی فقط پس از benchmark مستقل چندپروژه‌ای/چندسخت‌افزاری مجاز است.

### 0.15.0 — 2026-09-02

- تحویل Point-in-Time Illuminance مستقیم/پخشیده/کل و Daylight Factor با آسمان CIE Overcast؛
- تحویل annual climate-based daylight از EPW شامل sDA، direct-only ASE، چهار بازه UDI، timeline کامل و area-weighted project metrics؛
- تحویل Optical Material System واقعی با RGB reflectance/transmittance، پنج primitive سازگار Radiance، conservation validation و assignment دقیق با Object ID؛
- تحویل daylight coefficient matrix قطعی با cache در حافظه، reuse دقیق، hash و نمایش cache hit در Grasshopper؛
- تحویل Radiance export دقیق از instancing جهان به metre شامل material/geometry/sensors/manifest؛
- تحویل runner قابل لغو برای `oconv/rtrace` و annual matrix workflow با `rfluxmtx/gendaymtx/dctimestep`، cache مستقل از weather و provenance کامل command/tool/runtime/hash؛
- تحویل مقایسه Fast Path/Radiance شامل bias، MAE، RMSE، MAPE، max error، R² و acceptance envelope صریح؛
- افزودن هفت CLI، C ABI افزایشی، API کامل .NET 8/10 و پنج کامپوننت `XV Optical`، `XV Daylight`، `XV Annual Daylight`، `XV Radiance` و `XV Daylight Δ`؛
- ارتقای ABI/package/schema به `0.15.0` و gate runtime یکسان 38 کامپوننت برای Rhino 8/.NET 8 و Rhino 9/.NET 10.

**مرز علمی 0.15:** Fast Path فاقد multi-bounce interreflection است و به‌تنهایی مدرک compliance با LM-83 محسوب نمی‌شود. Radiance روی میزبان این انتشار نصب نبود؛ بنابراین export/runner/provenance پیاده‌سازی و تست شده‌اند ولی اجرای binary خارجی روی همان میزبان ادعا نمی‌شود. برتری عمومی فقط با corpus مستقل، پروژه‌های واقعی و Radiance version-pinned مجاز است.

### 0.16.0 — 2026-09-02

- تحویل Study Manifest رسمی Draft 2020-12 با parameter/objective scalar-vector/constraint/baseline/optimizer/metadata registry؛
- تحویل variant/evaluation ledger hash-protected، checkpoint کامل optimizer و PRNG و pending work، journal recovery و batch resume/commit اتمیک و idempotent؛
- تحویل constraint-aware Pareto و sensitivity استنباطی شامل Spearman، bootstrap CI، permutation p-value و Holm correction؛
- تحویل scenario comparison و delta map برای metric fieldهای هم‌تراز؛
- تحویل JSON کامل، Apache Parquet typed/Snappy، glTF 2.0 embedded-buffer، HTML report و standalone offline viewer؛
- افزودن شش CLI، C ABI UTF-8 افزایشی، API کامل .NET 8/10 و چهار کامپوننت `XV Manifest`، `XV Workspace`، `XV Commit` و `XV Report`؛
- ارتقای ABI/package/schema به `0.16.0` و gate runtime یکسان 42 کامپوننت برای Rhino 8/.NET 8 و Rhino 9/.NET 10.

**مرز علمی 0.16:** sensitivity رابطهٔ رتبه‌ای در sample ارزیابی‌شده است، نه علیت یا Sobol/Morris. workspace یک writer معتبر دارد و distributed database نیست. viewer مستقل و offline است، نه سرویس همکاری میزبانی‌شده. ادعای برتری optimizer یا محصول تا benchmark مستقل، پروژه واقعی، user study و reviewer خارجی مجاز نیست.

### 0.17.0 — 2026-09-03

- تحویل serialization مستقل صحنه با JSON schema، checksummed `.xvscene`، migration قطعی، stable identity و تبدیل OBJ/STL/PLY/glTF/GLB بدون Rhino؛
- تحویل interoperability زمان/هوا با WEA و CSV lossless، policy صریح missing/leap/gap/filter/occupancy و workflowهای headless؛
- تحویل policy یکپارچهٔ `XV Quality`، `XV Units`، `XV Diagnostics`، Basic/Expert، Live/Manual، debounce، stale result، cancellation/progress و legend مقاوم؛
- تکمیل Solar/Shading Envelope با axis آزاد برای هر candidate و تکمیل Visibility Graph sparse با spatial index، radius، degree cap و سقف 100,000 node؛
- افزودن soak قطعی، malformed-binary mutation/truncation، managed coverage حداقل 85 درصد و runtime load واقعی 46 کامپوننت در Rhino 8 و Rhino 9؛
- ارتقای موتور، ABI، managed assemblies، CLI، citation و package به `0.17.0` با حفظ مستقل schema دادهٔ Study روی `0.16.0` برای compatibility؛
- تحویل ZIP/CLI/SDK، بسته‌های Yak استاندارد `rh8_21` و `rh9_0`، checksum، release manifest، installer محلی، چهار سند onboarding واقعی Grasshopper و evidence خام GPU روی NVIDIA و Intel.

**مرز علمی/انتشار 0.17:** benchmark فعلی یک microbenchmark تحلیلی دو مثلثی برای parity و مشاهدهٔ transfer/runtime است و speedup عمومی یا برتری نسبت به رقبا را ثابت نمی‌کند. AMD، sceneهای معماری نماینده، beta project خارجی، user study و reviewer مستقل هنوز gate تجربی انتشار عمومی‌اند. بسته‌ها local release candidate هستند و هنوز روی package server عمومی upload نشده‌اند.

### 0.18.0 — 2026-09-04

- تحویل RASHNU Evidence Passport با هویت result/scene/input، method/fidelity، backend/device/tool، seed/sample/runtime، assumption/limitation/citation، evidence مربوط به parity/reference و BLAKE3 seal حساس به هر تغییر؛
- تحویل design واقعی Saltelli/Jansen با ماتریس‌های مستقل A/B/A_B(i) و design واقعی Morris با trajectoryهای OAT، row identity، hash alignment و estimator مخصوص هر روش؛
- تحویل robust scenario aggregation شامل weighted mean/std، worst، direction-aware upper-loss-tail CVaR و احتمال نقض constraint؛
- تحویل uncertainty-aware feasibility و interval Pareto که dominance را فقط با confidence intervalهای نامتقاطع می‌پذیرد؛
- تحویل multi-fidelity Fast/Reference calibration شامل intercept/slope/residual σ/R²، انتشار noise، Monte-Carlo EHVI دوبعدی و انتخاب reference job بر مبنای acquisition/cost؛
- افزودن شش CLI، شش C ABI sizing-pass، API کامل .NET 8/10، schemaهای Draft 2020-12 و سه کامپوننت `XV Evidence`، `XV Sensitivity+` و `XV Fidelity`؛
- افزودن analytic validation برای Sobol 0.2/0.8، Morris gradient، CVaR/chance constraint، interval rank و calibration/selection و اتصال آن به full check؛
- ارتقای engine/ABI/package/citation به `0.18.0`، با حفظ Study Manifest روی schema مستقل `0.16.0` و افزایش runtime gate به 49 کامپوننت Rhino 8/9.

**مرز علمی/انتشار 0.18:** policy فعلی یک linear discrepancy model شفاف با Monte-Carlo EHVI دوبعدی است و qNEHVI، MF-HVKG، Gaussian Process یا joint-batch fantasies ادعا نمی‌شود. analytic correctness جای corpus معماری، مقایسه مستقل Radiance، AMD evidence، پروژه beta، user study و reviewer مستقل را نمی‌گیرد؛ این موارد همچنان gate تجربی 1.0 هستند.

### 0.19.0 — 2026-09-04

- تحویل connector مستقل `Xvarna.Grasshopper2.rhp` روی .NET 10 و pin دقیق SDK رسمی prerelease مربوط به Rhino 9؛
- تحویل هر 49 قابلیت موجود GH1 در GH2، شامل setup/diagnostics، ZAMYAD/VAYU، solar/sky/irradiance/surface، DAENA، daylight/Radiance، Study/optimization/report و Evidence/multi-fidelity؛
- برابری کامل مجموعهٔ شناسه‌های معنایی: `IoId` هر قابلیت GH2 همان `ComponentGuid` قابلیت متناظر GH1 است؛
- انتقال `XvarnaSolarPeriod` به قرارداد host-neutral مشترک و جلوگیری از دو تعریف زمانی متفاوت؛
- ورودی‌های native برای mesh/point/vector/transform/twig و requestهای JSON نسخه‌پذیر برای رکوردهای علمی پر‌بعد، همراه result object تایپ‌شده، JSON کامل و hash؛
- افزودن runtime gate ایزوله برای load و construction هر 49 کامپوننت و contract probe واقعی برای ray، surface grid، Sky View، daylight، Target View، Pareto و Evidence؛
- اتصال GH2 به solution، full check، بستهٔ مشترک Rhino 9 GH1/GH2، release manifest و مثال‌های request قابل کپی.

**مرز سازگاری/انتشار 0.19:** SDK مربوط به GH2 هنوز prerelease است. gate خودکار registration، semantic identity و چند مسیر عددی end-to-end را اثبات می‌کند، نه تمام رفتارهای canvas. placement، wiring/twig، save/reopen، preview، progress/cancellation و نصب Yak در خود Rhino 9 باید پیش از public upload به‌صورت interactive قبول شوند. Rhino 8 + GH2 و یکسان‌بودن layout ورودی‌های GH1/GH2 ادعا نمی‌شود؛ semantics و engine مشترک‌اند.

---

## Appendix E — Requirement Traceability Matrix سطح کلان

| گروه نیازمندی | Work Package | Verification | Validation | مستند/مثال |
|---|---|---|---|---|
| `CORE-*` | WP-01 تا WP-04 | unit/property/integration/stress | scene واقعی | Core/Scene guide |
| `TIME-*` | WP-05 | parser/sun-position/golden | EPW/reference cases | Weather tutorial |
| `SOL-*` | WP-05/WP-09 | analytic/differential | Radiance و case study | Solar tutorials |
| `DL-*` | WP-11 | pipeline/metric tests | Radiance/LM-83 method | Daylight tutorial/report |
| `VIS-*` | WP-09 | analytic/graph/reference | case study و user task | Sight tutorials |
| `ATTR-*` | WP-05/WP-09 | conservation/top-K tests | user study | Attribution guide |
| `STUDY-*` | WP-10 | schema/resume/ranking tests | design options case | Study tutorial |
| `REP-*` | WP-10/WP-13 | snapshot/schema/link tests | reviewer usability | Reporting guide |
| `SEC-*` | همه، تمرکز WP-13 | fuzz/audit/negative tests | security review | SECURITY.md |
| `GOAL/KPI-*` | WP-14 | release dashboard | beta/benchmark | release report |

ردیابی جزئی‌تر در issue tracker انجام می‌شود. هر issue feature باید حداقل یک requirement ID و یک test/evidence artifact داشته باشد.

---

## Appendix F — تصویب و وضعیت مبنا

| نقش | نام | وضعیت | تاریخ | یادداشت |
|---|---|---|---|---|
| Product/Research Owner | مؤسس پروژه | Approved | 2026-08-30 | شروع اجرای پروژه تأیید شد |
| Engineering Owner | مؤسس/Lead Engineer | Approved for WP-00 | 2026-08-30 | target matrix Rhino 8/9 ثبت شد |
| Daylight Reviewer | تعیین می‌شود | Not Assigned | — | پیش از Gate 5 |
| GPU Reviewer | تعیین می‌شود | Not Assigned | — | پیش از Gate 4 |
| UX Reviewer | تعیین می‌شود | Not Assigned | — | پیش از Gate 6 |

پس از تأیید مالک پروژه، وضعیت بالای سند از «مبنای هنجاری و سند کار» به `Baseline Approved` تغییر می‌کند. تغییر عمده پس از آن از مسیر RFC انجام می‌شود.
