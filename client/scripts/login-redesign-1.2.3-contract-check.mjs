import fs from "node:fs";
import path from "node:path";
const root=process.cwd();
const tsx=fs.readFileSync(path.join(root,"src/pages/LoginPage.tsx"),"utf8");
const css=fs.readFileSync(path.join(root,"src/pages/LoginPage.css"),"utf8");
const errors=[];
function need(source,needle,label){if(!source.includes(needle))errors.push(`${label}: ${needle}`)}
for(const [needle,label] of [
 ['import "./LoginPage.css"','css import missing'],['className="login-page"','login-page class missing'],['className="login-shell"','login-shell class missing'],['className="login-card"','login-card class missing'],['className="login-visual"','login-visual class missing'],['className="cloud-diagram"','cloud-diagram class missing'],['className="login-world"','1.2.3 world visual missing'],['className="privacy-card"','1.2.3 privacy card missing'],['login-data-card','1.2.3 data cards missing'],['Private photo cloud','brand subtitle missing'],['Fotoğraflarınız. Verileriniz. Sizin bulutunuz.','tagline missing'],['GÜVENLİ ERİŞİM','secure-access label missing'],['Hesabınıza Giriş Yapın','login heading missing'],['PhotoOS’a Giriş','submit label missing'],['YOUR PRIVATE CLOUD','hero eyebrow missing'],['Kendi sunucunuzda. Kendi kurallarınızla.','private-cloud copy missing'],['`${API_URL}/api/v1/login`','login endpoint drift'],['JSON.stringify({ email, password })','login request shape drift'],['localStorage.setItem("token", data.data.token)','token persistence drift'],['window.location.href = "/dashboard"','post-login redirect drift']])need(tsx,needle,label);
for(const [needle,label] of [['.login-world','world CSS missing'],['.privacy-card','privacy CSS missing'],['.login-data-card','data-card CSS missing'],['@media(max-width:900px)','mobile visual breakpoint missing'],['@media(max-width:390px)','390px breakpoint missing'],['overflow-x:hidden','horizontal overflow guard missing']])need(css,needle,label);
if(/fingerprint|parmak izi|qr code|qr-code|webauthn/i.test(tsx))errors.push('unsupported authentication control introduced');
if(/\/api\/v1\/(forgot|reset|remember|webauthn|qr)/i.test(tsx))errors.push('unsupported auth API introduced');
if(/<img\b/i.test(tsx))errors.push('image dependency introduced');
if(errors.length){console.error('LOGIN_123_CONTRACT=FAIL');for(const e of errors)console.error(`- ${e}`);process.exit(1)}
console.log('LOGIN_123_AUTH_FLOW_PRESERVED=PASS');
console.log('LOGIN_123_VISUAL_CONTRACT=PASS');
console.log('LOGIN_123_MOBILE_390_CONTRACT=PASS');
console.log('LOGIN_123_UNSUPPORTED_AUTH_CONTROLS=0');
console.log('LOGIN_123_CONTRACT=PASS');
