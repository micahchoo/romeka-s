import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { restoreLocale } from './i18n';
import { Shell } from './app/shell';
import './styles.css';

const rootElement = document.getElementById('root');
if (rootElement === null) {
  throw new Error('index.html is missing the #root element');
}

const render = (): void => {
  createRoot(rootElement).render(
    <StrictMode>
      <Shell />
    </StrictMode>,
  );
};

// The stored locale's catalogue arrives before the first render, so a reload
// in Spanish never paints English first. A catalogue that fails to load
// leaves the admin in English rather than blank.
void restoreLocale()
  .catch((error: unknown) => {
    console.error(error);
  })
  .finally(render);
