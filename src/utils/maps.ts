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

export const countryName = (code?: string | null): string => {
  if (!code) return "";
  try {
    const dn = new Intl.DisplayNames(["zh-CN"], { type: "region" });
    return dn.of(code.toUpperCase()) ?? code;
  } catch {
    return code;
  }
};
