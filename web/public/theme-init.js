// 样式加载前定主题，避免刷新时闪白/闪黑。
// 只认 data-theme（不写 prefers-color-scheme 媒体查询块），令牌文件因此不必重复两遍。
(function () {
  try {
    var stored = localStorage.getItem('cicada.theme');
    var theme =
      stored === 'dark' || stored === 'light'
        ? stored
        : window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light';
    document.documentElement.dataset.theme = theme;
  } catch (error) {
    // 隐私模式下 localStorage 不可用：保持 index.html 里的 light 兜底。
    void error;
  }
})();
