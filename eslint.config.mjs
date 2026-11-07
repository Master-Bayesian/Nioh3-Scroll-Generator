import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['node_modules/**', '**/dist/**', '**/target/**', 'apps/tauri/src-tauri/**', 'apps/search-demo/**', 'third_party/**', 'research/**'] },
  { linterOptions: { reportUnusedDisableDirectives: 'off' } },
  {
    files: ['apps/**/*.{ts,tsx}', 'packages/**/*.ts'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: { '@typescript-eslint': tseslint.plugin },
    rules: {
      '@typescript-eslint/switch-exhaustiveness-check': ['error', { requireDefaultForNonUnion: false, considerDefaultExhaustiveForUnions: true }],
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-misused-promises': ['error', { checksVoidReturn: { attributes: false, arguments: false } }],
      '@typescript-eslint/await-thenable': 'error',
    },
  },
  {
    files: ['apps/desktop/tests/**'],
    rules: {
      '@typescript-eslint/no-floating-promises': 'off',
      '@typescript-eslint/no-misused-promises': 'off',
    },
  },
);
