import { createApp } from 'vue'
import App from './App.vue'

// Carbon Vue uses Carbon v10 classnames (bx--), so import matching v10 stylesheet.
import CarbonComponentsVue from '@carbon/vue'
import 'carbon-components/css/carbon-components.css'
import './style.css'

const app = createApp(App)

// Use the full Carbon Components Vue plugin
app.use(CarbonComponentsVue)

app.mount('#app')
