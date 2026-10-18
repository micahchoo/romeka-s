import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist/'] },
  {
    files: ['**/*.{ts,tsx}'],
    extends: [js.configs.recommended, ...tseslint.configs.strictTypeChecked],
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      '@typescript-eslint/consistent-type-assertions': ['error', { assertionStyle: 'never' }],
    },
  },
  {
    files: ['perf/**/*.mjs'],
    extends: [js.configs.recommended],
    languageOptions: {
      globals: globals.node,
    },
  },
  {
    files: ['tests/**/*.{ts,tsx}', 'e2e/**/*.ts', 'vite.config.ts'],
    languageOptions: {
      globals: globals.node,
    },
    rules: {
      // Playwright's fixture signature destructures the dependencies it needs, and
      // the `baseURL` override needs none of them.
      'no-empty-pattern': 'off',
    },
  },
);
