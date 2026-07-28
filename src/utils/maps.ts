export const gamemodeName = (playlist?: string | null): string => {
  switch (playlist) {
    case "fs_dm": return "FFA";
    case "fs_prophunt": return "Prophunt";
    case "fs_1v1": return "1v1";
    case "fs_duckhunt": return "Duckhunt";
    case "fs_mantlejumppractice": return "Mantle Jump";
    case "fs_infected": return "Infected";
    case "custom_tdm": return "TDM";
    case "custom_ctf": return "CTF";
    case "survival": return "Battle Royal";
    case "fs_movementgym": return "Movement Gym";
    case "fs_survival_solos": return "BR Solo";
    case "fs_vamp_1v1": return "Vamp 1v1";
    case "fs_realistic_ttv": return "Realistic FFA";
    case "fs_dm_fast_instagib": return "Instagib";
    default: return playlist || "";
  }
};

export const mapName = (map?: string | null): string => {
  switch (map) {
    case "mp_rr_canyonlands_staging": return "Firing Range";
    case "mp_rr_aqueduct": return "Overflow";
    case "mp_rr_aqueduct_night": return "Overflow After Dark";
    case "mp_rr_ashs_redemption": return "Ashs Redemption";
    case "mp_rr_canyonlands_64k_x_64k": return "Kings Canyon S1";
    case "mp_rr_canyonlands_mu1": return "Kings Canyon S2";
    case "mp_rr_canyonlands_mu1_night": return "Kings Canyon S2 After Dark";
    case "mp_rr_desertlands_64k_x_64k": return "Worlds Edge";
    case "mp_rr_desertlands_64k_x_64k_nx": return "Worlds Edge After Dark";
    case "mp_rr_desertlands_64k_x_64k_tt": return "Worlds Edge Mirage Voyage";
    case "mp_rr_arena_composite": return "Drop Off";
    case "mp_rr_arena_skygarden": return "Encore";
    case "mp_rr_party_crasher": return "Party Crasher";
    case "mp_lobby": return "Lobby";
    case "mp_rr_arena_phase_runner": return "Phase Runner";
    default: return map || "";
  }
};

const IMAGE_BASE_URL = "https://r5.sleep0.de/img";

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
  const normalized = code?.trim().toLowerCase() ?? "";
  const fileName = /^[a-z]{2}$/.test(normalized) ? normalized : "xx";
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
