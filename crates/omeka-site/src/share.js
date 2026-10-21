// Copy a manifest URL for the "Use elsewhere" block. The button carries every
// string it shows, so this file holds no text and needs no translation.
(() => {
  for (const button of document.querySelectorAll('.copy-manifest')) {
    const url = button.dataset.copyUrl;
    if (!url || button.dataset.bound === 'true') {
      continue;
    }
    button.dataset.bound = 'true';
    const item = button.closest('.use-elsewhere-manifest');
    const status = item?.querySelector('.copy-status');
    const input = item?.querySelector('.manifest-url');
    let timer;
    const restore = () => {
      button.textContent = button.dataset.label ?? '';
      if (status) {
        status.textContent = '';
      }
    };
    button.addEventListener('click', async () => {
      clearTimeout(timer);
      try {
        await navigator.clipboard.writeText(url);
        button.textContent = button.dataset.copied ?? '';
        if (status) {
          status.textContent = status.dataset.copied ?? '';
        }
      } catch {
        button.textContent = button.dataset.label ?? '';
        if (status) {
          status.textContent = button.dataset.copyFailed ?? '';
        }
        if (input instanceof HTMLInputElement) {
          input.focus();
          input.select();
        }
      }
      timer = setTimeout(restore, 2000);
    });
  }
})();
