for (const link of document.querySelectorAll('[data-download-next]')) {
  link.addEventListener('click', (event) => {
    if (event.defaultPrevented) return;
    const status = document.getElementById(link.dataset.downloadNext);
    if (status) status.textContent = link.dataset.downloadMessage ?? status.dataset.message;
  });
}
