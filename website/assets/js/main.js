/* Motr website — menu, locale switch and the confetti entrance. */
(function () {
  "use strict";

  var COPY = {
    en: {
      "hero.title": "A full-featured download manager",
      "hero.desc": "Support downloading HTTP, FTP, BitTorrent, Magnet, etc.",
      "hero.download": "Download",
      "hero.new": "New",
      "hero.features": "View Motr features",
      "hero.other": "It’s free and open source, Welcome fork or PR",
      "hero.poster": "assets/img/screenshot-task-list-downloading-en@2x.png",
      "hero.posterAlt": "Motr task list",
      "features.title": "Features",
      "feat.1": "Simple and clear user interface",
      "feat.2": "Support BitTorrent & Magnet",
      "feat.3": "BitTorrent selective download",
      "feat.4": "Update tracker list every day automatically",
      "feat.5": "UPnP & NAT-PMP Port Mapping",
      "feat.6": "Up to 10 tasks concurrently download",
      "feat.7": "Single task maximum support 64 thread download",
      "feat.8": "Download progress graphic dynamic display",
      "feat.9": "Mock User-Agent",
      "feat.10": "Download completed Notification",
      "feat.12": "Resident system tray for quick operation",
      "feat.13": "Dark mode",
      "feat.14": "Delete related files when removing tasks (optional)",
      "feat.15": "I18n",

    },
    zh: {
      "hero.title": "一款全能的下载工具",
      "hero.desc": "支持下载 HTTP、FTP、BT、磁力链接等资源",
      "hero.download": "立即下载",
      "hero.new": "New",
      "hero.features": "查看 Motr 特性介绍",
      "hero.other": "免费开源，欢迎 Fork 和 PR",
      "hero.poster": "assets/img/screenshot-task-list-downloading-zh@2x.png",
      "hero.posterAlt": "Motr 任务列表",
      "features.title": "特性",
      "feat.1": "简洁明了的图形操作界面",
      "feat.2": "支持BT和磁力链任务",
      "feat.3": "支持选择性下载BT部分文件",
      "feat.4": "每天自动更新 Tracker 服务器列表",
      "feat.5": "UPnP & NAT-PMP 端口映射",
      "feat.6": "最高支持 10 个任务同时下载",
      "feat.7": "单任务最高支持 64 线程下载",
      "feat.8": "下载进度图形动态显示",
      "feat.9": "模拟用户代理UA",
      "feat.10": "下载完成后通知",
      "feat.12": "常驻系统托盘，操作更加便捷",
      "feat.13": "深色模式",
      "feat.14": "移除任务时可同时删除相关文件",
      "feat.15": "国际化",

    }
  };

  var TITLES = {
    home: { en: "Motr - A full-featured download manager", zh: "Motr - 一款全能的下载工具" },
    features: { en: "Features - Motr", zh: "特性 - Motr" }
  };

  var STORAGE_KEY = "motr.locale";
  var page = document.body.getAttribute("data-page") || "home";
  var nodes = document.querySelectorAll("[data-i18n]");
  var srcNodes = document.querySelectorAll("[data-i18n-src]");
  var altNodes = document.querySelectorAll("[data-i18n-alt]");
  var localeLinks = document.querySelectorAll(".locales [data-lang]");

  function setLocale(lang) {
    var dict = COPY[lang] || COPY.en;

    nodes.forEach(function (node) {
      var key = node.getAttribute("data-i18n");
      if (dict[key]) node.textContent = dict[key];
    });

    srcNodes.forEach(function (node) {
      var key = node.getAttribute("data-i18n-src");
      if (dict[key]) node.src = dict[key];
    });

    altNodes.forEach(function (node) {
      var key = node.getAttribute("data-i18n-alt");
      if (dict[key]) node.setAttribute("alt", dict[key]);
    });

    document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
    document.title = (TITLES[page] || TITLES.home)[lang] || TITLES.home.en;

    localeLinks.forEach(function (link) {
      link.classList.toggle("locale-active", link.getAttribute("data-lang") === lang);
    });

    try {
      localStorage.setItem(STORAGE_KEY, lang);
    } catch (error) {
      /* private mode */
    }
  }

  function initialLocale() {
    var query = /[?&]lang=([\w-]+)/.exec(window.location.search);
    if (query) return query[1].toLowerCase().indexOf("zh") === 0 ? "zh" : "en";

    try {
      var saved = localStorage.getItem(STORAGE_KEY);
      if (saved === "en" || saved === "zh") return saved;
    } catch (error) {
      /* ignore */
    }

    return (navigator.language || "en").toLowerCase().indexOf("zh") === 0 ? "zh" : "en";
  }

  var locale = initialLocale();
  setLocale(locale);

  localeLinks.forEach(function (link) {
    link.addEventListener("click", function (event) {
      event.preventDefault();
      locale = link.getAttribute("data-lang");
      setLocale(locale);
    });
  });

  /* --------------------------------------------------------------- confetti */

  var shapes = document.querySelectorAll(".shapes > *");
  var reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  if (shapes.length && !reduce) {
    shapes.forEach(function (shape, index) {
      var target = shape.style.transform;
      var opacity = shape.style.opacity || "0.6";

      shape.style.transition =
        "transform 900ms cubic-bezier(0.2, 0.8, 0.3, 1) " +
        (60 + (index % 14) * 34) +
        "ms, opacity 700ms linear " +
        (60 + (index % 14) * 34) +
        "ms";
      shape.style.transform = "translate(0px, 0px) scale(0.01)";
      shape.style.opacity = "0";

      window.requestAnimationFrame(function () {
        window.requestAnimationFrame(function () {
          shape.style.transform = target;
          shape.style.opacity = opacity;
        });
      });
    });
  }

})();
