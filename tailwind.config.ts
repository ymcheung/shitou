import forms from '@tailwindcss/forms';
import typography from '@tailwindcss/typography';
import type { Config } from 'tailwindcss';

export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  darkMode: 'class',
  theme: {
    extend: {
      fontFamily: {
        sans: [
          '-apple-system',
          'BlinkMacSystemFont',
          'SF Pro Text',
          'Inter',
          'Segoe UI',
          'sans-serif'
        ]
      },
      boxShadow: {
        soft: '0 18px 45px rgba(24, 24, 27, 0.12)',
        panel: '0 1px 2px rgba(24, 24, 27, 0.08), 0 20px 50px rgba(24, 24, 27, 0.08)'
      }
    }
  },
  plugins: [forms, typography]
} satisfies Config;
