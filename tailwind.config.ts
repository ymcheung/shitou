import forms from '@tailwindcss/forms';
import typography from '@tailwindcss/typography';
import type { Config } from 'tailwindcss';

export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        ink: {
          50: '#f8fafc',
          100: '#eef2f7',
          200: '#d9e2ec',
          300: '#bcccdc',
          400: '#829ab1',
          500: '#627d98',
          600: '#486581',
          700: '#334e68',
          800: '#243b53',
          900: '#102a43'
        },
        signal: {
          500: '#2563eb',
          600: '#1d4ed8'
        }
      },
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
        soft: '0 12px 36px rgba(15, 23, 42, 0.12)'
      }
    }
  },
  plugins: [forms, typography]
} satisfies Config;
