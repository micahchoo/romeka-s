import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { applyDocumentLang } from './i18n';
import { Shell } from './app/shell';
import './styles.css';

const rootElement = document.getElementById('root');
if (rootElement === null) {
  throw new Error('index.html is missing the #root element');
}

// The stored locale, not the `lang="en"` in `index.html`, is the document's
// language from the first render.
applyDocumentLang();

createRoot(rootElement).render(
  <StrictMode>
    <Shell />
  </StrictMode>,
);
