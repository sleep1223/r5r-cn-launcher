export const gamemodeName = (playlist?: string | null): string => {
  switch (playlist) {
    case "fs_dm": return "自由混战";
    case "fs_prophunt": return "道具躲猫猫";
    case "fs_1v1": return "1 对 1";
    case "fs_duckhunt": return "猎鸭";
    case "fs_mantlejumppractice": return "攀爬跳练习";
    case "fs_infected": return "感染模式";
    case "custom_tdm": return "团队死斗";
    case "custom_ctf": return "夺旗";
    case "survival": return "大逃杀";
    case "fs_movementgym": return "身法训练场";
    case "fs_survival_solos": return "单人赛";
    case "fs_vamp_1v1": return "吸血 1 对 1";
    case "fs_realistic_ttv": return "拟真自由混战";
    case "fs_dm_fast_instagib": return "一击必杀";
    default: return playlist || "";
  }
};

export const mapName = (map?: string | null): string => {
  switch (map) {
    case "mp_rr_canyonlands_staging": return "射击场";
    case "mp_rr_aqueduct": return "熔岩流";
    case "mp_rr_aqueduct_night": return "夜间熔岩流";
    case "mp_rr_ashs_redemption": return "艾许的救赎";
    case "mp_rr_canyonlands_64k_x_64k": return "诸王峡谷（第 1 赛季）";
    case "mp_rr_canyonlands_mu1": return "诸王峡谷（第 2 赛季）";
    case "mp_rr_canyonlands_mu1_night": return "夜间诸王峡谷（第 2 赛季）";
    case "mp_rr_desertlands_64k_x_64k": return "世界尽头";
    case "mp_rr_desertlands_64k_x_64k_nx": return "夜间世界尽头";
    case "mp_rr_desertlands_64k_x_64k_tt": return "世界尽头：幻象游轮";
    case "mp_rr_arena_composite": return "原料场";
    case "mp_rr_arena_skygarden": return "再来一次";
    case "mp_rr_party_crasher": return "派对破坏者";
    case "mp_lobby": return "大厅";
    case "mp_rr_arena_phase_runner": return "相位穿梭器";
    default: return map || "";
  }
};

const IMAGE_BASE_URL = "https://r5.sleep0.de/img";
const COUNTRY_CODES =
  "AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI VN VU WF WS XK YE YT ZA ZM ZW".split(
    " ",
  );

const normalizeCountryLookup = (value: string): string =>
  value
    .trim()
    .toLocaleLowerCase()
    .replace(/特别行政区$/u, "")
    .replace(/[\s·,，.'’()（）-]/gu, "");

const COUNTRY_CODE_ALIASES: Record<string, string> = {
  中国台湾: "TW",
  台湾省: "TW",
  台湾地区: "TW",
  香港: "HK",
  中国香港: "HK",
  澳门: "MO",
  中国澳门: "MO",
};

let countryCodeByName: Map<string, string> | null = null;

export const countryCodeFromName = (
  value?: string | null,
): string | null => {
  const raw = value?.trim() ?? "";
  if (!raw) return null;

  const code = raw.toUpperCase();
  if (/^[A-Z]{2}$/.test(code)) return code;

  const normalized = normalizeCountryLookup(raw);
  const alias = COUNTRY_CODE_ALIASES[normalized];
  if (alias) return alias;

  if (!countryCodeByName) {
    countryCodeByName = new Map<string, string>();
    for (const locale of ["zh-CN", "en"]) {
      const displayNames = new Intl.DisplayNames([locale], { type: "region" });
      for (const countryCode of COUNTRY_CODES) {
        try {
          const name = displayNames.of(countryCode);
          if (name) {
            countryCodeByName.set(
              normalizeCountryLookup(name),
              countryCode,
            );
          }
        } catch {
          // Ignore non-standard codes unsupported by the current runtime.
        }
      }
    }
  }

  return countryCodeByName.get(normalized) ?? null;
};

export const mapImage = (map?: string | null): string => {
  switch (map) {
    case "mp_rr_canyonlands_staging": return `${IMAGE_BASE_URL}/maps/mp_rr_canyonlands_staging.webp`;
    case "mp_rr_aqueduct": return `${IMAGE_BASE_URL}/maps/mp_rr_aqueduct.webp`;
    case "mp_rr_aqueduct_night": return `${IMAGE_BASE_URL}/maps/mp_rr_aqueduct_night.webp`;
    case "mp_rr_ashs_redemption": return `${IMAGE_BASE_URL}/maps/mp_rr_ashs_redemption.webp`;
    case "mp_rr_canyonlands_64k_x_64k": return `${IMAGE_BASE_URL}/maps/mp_rr_canyonlands_64k_x_64k.webp`;
    case "mp_rr_canyonlands_mu1": return `${IMAGE_BASE_URL}/maps/mp_rr_canyonlands_mu1.webp`;
    case "mp_rr_canyonlands_mu1_night": return `${IMAGE_BASE_URL}/maps/mp_rr_canyonlands_mu1_night.webp`;
    case "mp_rr_desertlands_64k_x_64k": return `${IMAGE_BASE_URL}/maps/mp_rr_desertlands_64k_x_64k.webp`;
    case "mp_rr_desertlands_64k_x_64k_nx": return `${IMAGE_BASE_URL}/maps/mp_rr_desertlands_64k_x_64k_nx.webp`;
    case "mp_rr_desertlands_64k_x_64k_tt": return `${IMAGE_BASE_URL}/maps/mp_rr_desertlands_64k_x_64k_tt.png`;
    case "mp_rr_arena_composite": return `${IMAGE_BASE_URL}/maps/mp_rr_arena_composite.webp`;
    case "mp_rr_arena_skygarden": return `${IMAGE_BASE_URL}/maps/mp_rr_arena_skygarden.webp`;
    case "mp_rr_party_crasher": return `${IMAGE_BASE_URL}/maps/mp_rr_party_crasher.webp`;
    case "mp_rr_arena_phase_runner": return `${IMAGE_BASE_URL}/maps/mp_rr_arena_phase_runner.png`;
    case "mp_rr_olympus_mu1": return `${IMAGE_BASE_URL}/maps/mp_rr_olympus_mu1.png`;
    case "mp_flowstate": return `${IMAGE_BASE_URL}/maps/mp_flowstate.png`;
    case "mp_rr_arena_empty": return `${IMAGE_BASE_URL}/maps/mp_rr_arena_empty.png`;
    case "mp_rr_construct": return `${IMAGE_BASE_URL}/maps/mp_rr_construct.png`;
    case "mp_rr_desertlands_holiday": return `${IMAGE_BASE_URL}/maps/mp_rr_desertlands_holiday.png`;
    default: return `${IMAGE_BASE_URL}/maps/default.png`;
  }
};

export const flagImage = (code?: string | null): string => {
  const fileName = countryCodeFromName(code)?.toLowerCase() ?? "xx";
  return `${IMAGE_BASE_URL}/flags/${fileName}.svg`;
};

const WEAPON_ZH: Record<string, string> = {
  alternator: "转换者冲锋枪",
  "charge rifle": "充能步枪",
  devotion: "专注冲锋枪",
  epg: "EPG",
  eva8: "EVA8",
  flatline: "平行步枪",
  g7: "G7侦察枪",
  havoc: "哈沃克步枪",
  hemlok: "赫姆洛克突击步枪",
  kraber: "克雷贝尔狙击枪",
  longbow: "长弓狙击步枪",
  lstar: "L-STAR能量机枪",
  mastiff: "敖犬霰弹枪",
  mozambique: "莫桑比克",
  p2020: "P2020",
  peacekeeper: "和平捍卫者",
  prowler: "猎兽冲锋枪",
  r301: "R301步枪",
  r99: "R99冲锋枪",
  re45: "RE45手枪",
  "smart pistol": "智慧手枪",
  spitfire: "喷火轻机枪",
  "triple take": "三重式狙击枪",
  wingman: "辅助手枪",
  volt: "电能冲锋枪",
  crossbow_bolt: "波塞克弓",
  trigger_hurt: "致命区域",
  player: "近战",
};

export const weaponName = (weapon?: string | null): string => {
  if (!weapon) return "";
  return WEAPON_ZH[weapon.toLowerCase()] ?? weapon;
};

export const countryName = (code?: string | null): string => {
  if (!code) return "";
  try {
    const dn = new Intl.DisplayNames(["zh-CN"], { type: "region" });
    return dn.of(code.toUpperCase()) ?? code;
  } catch {
    return code;
  }
};

export const countryFlag = (code?: string | null): string => {
  const normalized = code?.trim().toUpperCase() ?? "";
  if (!/^[A-Z]{2}$/.test(normalized)) return "🌐";
  return String.fromCodePoint(
    ...[...normalized].map((char) => 127397 + char.charCodeAt(0)),
  );
};
