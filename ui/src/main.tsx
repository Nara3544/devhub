import { createRoot } from 'react-dom/client'
import '@astryxdesign/core/reset.css'
import '@astryxdesign/core/astryx.css'
import '@astryxdesign/theme-neutral/theme.css'
import './styles.css'
import { Theme, defineTheme } from '@astryxdesign/core/theme'
import { neutralTheme } from '@astryxdesign/theme-neutral/built'
import { App } from './App'

const JP = '"Yu Gothic UI", "Yu Gothic", Meiryo, system-ui, sans-serif'
const theme = defineTheme({
  name: 'devhub',
  extends: neutralTheme,
  tokens: { '--font-family-body': JP, '--font-family-heading': JP },
})

const dark = window.matchMedia('(prefers-color-scheme: dark)').matches

createRoot(document.getElementById('root')!).render(
  <Theme theme={theme} mode={dark ? 'dark' : 'light'}>
    <App />
  </Theme>,
)
