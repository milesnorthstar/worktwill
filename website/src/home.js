import messages from './home.generated.js';
document.documentElement.classList.add('home-js');
const t = (text) => document.documentElement.lang === 'en' ? messages[text] : text;
    const toast = document.getElementById("toast");
    let toastTimer;
    function showToast(message) {
      toast.textContent = message;
      toast.classList.add("show");
      clearTimeout(toastTimer);
      toastTimer = setTimeout(() => toast.classList.remove("show"), 2600);
    }

    // Mobile navigation
    const header = document.getElementById("siteHeader");
    const menuToggle = document.getElementById("menuToggle");
    menuToggle.addEventListener("click", () => {
      const open = header.classList.toggle("menu-open");
      menuToggle.setAttribute("aria-expanded", String(open));
      menuToggle.textContent = open ? "×" : "☰";
    });
    document.querySelectorAll(".nav-links a").forEach(link => link.addEventListener("click", () => {
      header.classList.remove("menu-open");
      menuToggle.setAttribute("aria-expanded", "false");
      menuToggle.textContent = "☰";
    }));

    // Scroll reveal and progress
    const revealObserver = new IntersectionObserver(entries => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          entry.target.classList.add("visible");
          revealObserver.unobserve(entry.target);
        }
      });
    }, { threshold: .13 });
    document.querySelectorAll(".reveal").forEach(el => revealObserver.observe(el));

    const scrollLine = document.getElementById("scrollLine");
    function updateScroll() {
      const max = document.documentElement.scrollHeight - window.innerHeight;
      scrollLine.style.width = (max > 0 ? window.scrollY / max * 100 : 0) + "%";
    }
    window.addEventListener("scroll", updateScroll, { passive: true });
    updateScroll();

    // Cursor light
    const cursorGlow = document.getElementById("cursorGlow");
    let glowFrame = 0;
    window.addEventListener("pointermove", event => {
      if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
      if (glowFrame) cancelAnimationFrame(glowFrame);
      glowFrame = requestAnimationFrame(() => {
        cursorGlow.style.left = event.clientX + "px";
        cursorGlow.style.top = event.clientY + "px";
      });
    }, { passive: true });

    // Hero perspective follows pointer, with restrained movement.
    const heroVisual = document.getElementById("heroVisual");
    const appWindow = document.getElementById("appWindow");
    heroVisual.addEventListener("pointermove", event => {
      if (window.matchMedia("(prefers-reduced-motion: reduce)").matches || window.innerWidth < 900) return;
      const rect = heroVisual.getBoundingClientRect();
      const x = (event.clientX - rect.left) / rect.width - .5;
      const y = (event.clientY - rect.top) / rect.height - .5;
      appWindow.style.transform = `rotateY(${-7 + x * 5}deg) rotateX(${2 - y * 4}deg) rotateZ(${-1 + x * .7}deg) translateY(-2px)`;
    });
    heroVisual.addEventListener("pointerleave", () => appWindow.style.removeProperty("transform"));

    // Search interaction: the hero search opens the inline result state.
    const heroSearch = document.getElementById("heroSearch");
    const heroSearchText = document.getElementById("heroSearchText");
    let searchOpen = false;
    function toggleHeroSearch() {
      searchOpen = !searchOpen;
      heroSearchText.textContent = searchOpen ? t("品牌企划") : t("搜索文件、网页和工作区…");
      heroSearch.style.borderColor = searchOpen ? "rgba(173,164,255,.7)" : "rgba(203,213,255,.13)";
      heroSearch.style.background = searchOpen ? "rgba(105,95,210,.2)" : "rgba(3,8,23,.3)";
      showToast(searchOpen ? t("搜索面板已展开 · 全部资源") : t("搜索面板已关闭"));
    }
    heroSearch.addEventListener("click", toggleHeroSearch);
    heroSearch.addEventListener("keydown", e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleHeroSearch(); } });

    // Live search demo changes the matching result list.
    const demoInput = document.getElementById("demoInput");
    const demoResults = document.getElementById("demoResults");
    const demoData = [
      { name:t("品牌企划"), desc:t("工作区 · 12 份资料"), type:t("工作区"), symbol:"▰", keys:[t("品牌"),t("企划")] },
      { name:t("品牌企划预算.xlsx"), desc:t("文档 / 项目 / 预算"), type:t("文件"), symbol:"X", keys:[t("品牌"),t("企划"),t("预算")] },
      { name:t("品牌视觉参考"), desc:"https://are.na/…", type:t("网页"), symbol:"↗", keys:[t("品牌"),t("视觉"),t("参考")] },
      { name:t("季度运营复盘.docx"), desc:t("文档 / 运营 / 复盘"), type:t("文件"), symbol:"W", keys:[t("运营"),t("复盘")] },
      { name:t("产品需求与路线图.pdf"), desc:t("文档 / 产品 / 规划"), type:t("文件"), symbol:"P", keys:[t("产品"),t("需求"),t("路线图")] }
    ];
    function renderDemoResults(query) {
      const q = query.trim().toLowerCase();
      const matches = demoData.filter(item => !q || item.name.toLowerCase().includes(q) || item.desc.toLowerCase().includes(q) || item.keys.some(k => k.includes(q) || q.includes(k)));
      demoResults.innerHTML = "";
      if (!matches.length) {
        const empty = document.createElement('div');
        empty.style.cssText = 'padding:24px 10px;text-align:center;color:#8c93a9;font-size:12px';
        empty.textContent = t("没有匹配的演示结果");
        demoResults.append(empty);
        return;
      }
      matches.slice(0,4).forEach((item,index) => {
        const row = document.createElement("div");
        row.className = "demo-result" + (index === 0 ? " selected" : "");
        row.innerHTML = `<span class="result-symbol">${item.symbol}</span><span><strong>${item.name}</strong><small>${item.desc}</small></span><span class="kind">${item.type}</span>`;
        row.addEventListener("click", () => showToast(t("演示：打开「") + item.name + (document.documentElement.lang === 'en' ? '”' : '」')));
        demoResults.appendChild(row);
      });
    }
    demoInput.addEventListener("input", () => renderDemoResults(demoInput.value));
    demoInput.addEventListener("keydown", e => {
      if (e.key === "Enter") showToast(t("演示：已选择搜索结果"));
      if (e.key === "Escape") { demoInput.value = ""; renderDemoResults(""); demoInput.blur(); }
    });

    // FAQ accordion
    document.querySelectorAll(".faq-question").forEach(button => {
      button.addEventListener("click", () => {
        const item = button.closest(".faq-item");
        const willOpen = !item.classList.contains("open");
        document.querySelectorAll(".faq-item").forEach(other => {
          other.classList.remove("open");
          other.querySelector(".faq-question").setAttribute("aria-expanded", "false");
        });
        if (willOpen) {
          item.classList.add("open");
          button.setAttribute("aria-expanded", "true");
        }
      });
    });


// 平台不可用时保留原型提示；公开源码入口使用真实产品说明页。
document.querySelector("#windowsDownload")?.addEventListener("click", () => { if (document.querySelector("#windowsDownload").tagName === "BUTTON") showToast(t("Windows 安装包尚未发布。")); });
