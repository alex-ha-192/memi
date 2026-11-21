package app.memi.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * Every colour the app uses. Each scheme comes in a light and a dark version; the shapes,
 * spacing and type stay the same so every scheme still feels like Bento.
 */
data class BentoColors(
    val isDark: Boolean,
    val bg: Color,
    val ink: Color,
    val muted: Color,
    val card: Color,
    val line: Color,
    val sheet: Color,
    val handle: Color,
    val track: Color,
    val dock: Color,
    val dockInk: Color,
    val dockMuted: Color,
    val dockChip: Color,
    /** Tile and tag colours for the five collection slots: gift, plan, note, extra A, extra B. */
    val tiles: List<Color>,
    val tileInks: List<Color>,
    /** Accent strips on cards, one per slot. */
    val strips: List<Color>,
    /** Placeholder tones for people without a photo. */
    val tones: List<Color>,
    /** Look A4: each tone's card background and secondary-text colour, alternating (see PersonCards.kt). */
    val personCards: List<Color> = emptyList(),
    val hit: Color,
    val hitInk: Color,
    val miss: Color,
    val missInk: Color,
    val danger: Color,
    val scrim: Color,
) {
    val gift get() = tiles[0]
    val giftInk get() = tileInks[0]
    val plan get() = tiles[1]
    val planInk get() = tileInks[1]
    val note get() = tiles[2]
    val noteInk get() = tileInks[2]

    /** The tinted card behind a person, from their tone. */
    fun personCard(toneIndex: Int): Color = personCards.getOrNull(2 * toneIndex.mod(8)) ?: card
    /** Their secondary line on that card: 4.5:1 contrast or better. */
    fun personInk(toneIndex: Int): Color = personCards.getOrNull(2 * toneIndex.mod(8) + 1) ?: muted
}

data class Scheme(val name: String, val light: BentoColors, val dark: BentoColors)

private fun scheme(name: String, light: BentoColors, dark: BentoColors) = Scheme(
    name,
    light.copy(personCards = PERSON_CARDS.getValue("$name light").map(::c)),
    dark.copy(personCards = PERSON_CARDS.getValue("$name dark").map(::c)),
)

private fun c(hex: Long) = Color(hex)

private fun light(
    bg: Long, ink: Long, muted: Long,
    gift: Long, giftInk: Long, plan: Long, planInk: Long, note: Long, noteInk: Long,
    extraA: Long, extraAInk: Long, extraB: Long, extraBInk: Long,
    strips: List<Long>, tones: List<Long>,
) = BentoColors(
    isDark = false,
    bg = c(bg), ink = c(ink), muted = c(muted),
    card = Color.White, line = c(ink).copy(alpha = 0.08f),
    sheet = Color.White, handle = c(ink).copy(alpha = 0.16f), track = c(bg),
    dock = c(ink), dockInk = Color.White, dockMuted = Color.White.copy(alpha = 0.62f),
    dockChip = Color.White.copy(alpha = 0.12f),
    tiles = listOf(c(gift), c(plan), c(note), c(extraA), c(extraB)),
    tileInks = listOf(c(giftInk), c(planInk), c(noteInk), c(extraAInk), c(extraBInk)),
    strips = strips.map(::c),
    tones = tones.map(::c),
    hit = c(plan), hitInk = c(planInk),
    miss = c(ink).copy(alpha = 0.08f), missInk = c(muted),
    danger = c(0xFFB42318),
    scrim = Color.Black.copy(alpha = 0.32f),
)

private fun dark(
    bg: Long, ink: Long, sheet: Long, dock: Long,
    gift: Long, giftInk: Long, plan: Long, planInk: Long, note: Long, noteInk: Long,
    extraA: Long, extraAInk: Long, extraB: Long, extraBInk: Long,
    strips: List<Long>, tones: List<Long>,
) = BentoColors(
    isDark = true,
    bg = c(bg), ink = c(ink), muted = c(ink).copy(alpha = 0.66f),
    card = Color.White.copy(alpha = 0.07f), line = Color.White.copy(alpha = 0.08f),
    sheet = c(sheet), handle = Color.White.copy(alpha = 0.18f), track = Color.White.copy(alpha = 0.06f),
    dock = c(dock), dockInk = c(ink), dockMuted = c(ink).copy(alpha = 0.6f),
    dockChip = Color.White.copy(alpha = 0.1f),
    tiles = listOf(c(gift), c(plan), c(note), c(extraA), c(extraB)),
    tileInks = listOf(c(giftInk), c(planInk), c(noteInk), c(extraAInk), c(extraBInk)),
    strips = strips.map(::c),
    tones = tones.map(::c),
    hit = c(plan), hitInk = c(planInk),
    miss = Color.White.copy(alpha = 0.08f), missInk = c(ink).copy(alpha = 0.66f),
    danger = c(0xFFFF8A80),
    scrim = Color.Black.copy(alpha = 0.5f),
)

object Schemes {
    val Bento = scheme(
        "Bento",
        light(
            0xFFF4F3F8, 0xFF1C1B2E, 0xFF5E5C74,
            0xFFFFDCCB, 0xFF8A3B17, 0xFFCFF0DE, 0xFF1E6B43, 0xFFFFF0C2, 0xFF7A5A00,
            0xFFE3DEFF, 0xFF4B3FD1, 0xFFD5E9FF, 0xFF1D5A9E,
            listOf(0xFFFFB899, 0xFF8FDDB4, 0xFFFFDC6B, 0xFFBDB3FF, 0xFF9CC8F5),
            listOf(0xFFE8B8A2, 0xFFA9CDB9, 0xFFBDB6E6, 0xFFE6CF95, 0xFFA9C6E4, 0xFFE4AFC4, 0xFFC9D29A, 0xFFB9C3D6),
        ),
        dark(
            0xFF15141E, 0xFFF4F3F8, 0xFF22212E, 0xFF2E2C40,
            0xFF4A3029, 0xFFFFC9B3, 0xFF1F3B2F, 0xFFA8E8C6, 0xFF3D3520, 0xFFF5DC8A,
            0xFF2E2A55, 0xFFC9C1FF, 0xFF1F3048, 0xFFAED3FF,
            listOf(0xFFB06A4C, 0xFF4E9C74, 0xFFB8963A, 0xFF7D70D6, 0xFF5D8FC7),
            listOf(0xFF7A5446, 0xFF4D6B5C, 0xFF5B5480, 0xFF7A6A44, 0xFF4C6680, 0xFF7A4D5E, 0xFF65703F, 0xFF56607A),
        ),
    )

    val Sorbet = scheme(
        "Sorbet",
        light(
            0xFFFBF4F1, 0xFF2B1D24, 0xFF6E5A62,
            0xFFFFD5DF, 0xFF8C2547, 0xFFDCF1CC, 0xFF2F6420, 0xFFFFE3C4, 0xFF7E4A12,
            0xFFE9DDFB, 0xFF5B3A9E, 0xFFD3ECF2, 0xFF1E5E6E,
            listOf(0xFFF59AB2, 0xFF9AD47C, 0xFFF8B878, 0xFFB79BEA, 0xFF86C3D2),
            listOf(0xFFE9AFBD, 0xFFB5D49E, 0xFFF2C29A, 0xFFC9B3E6, 0xFFA7CFD8, 0xFFE3B0A4, 0xFFD6C78F, 0xFFC4B6BE),
        ),
        dark(
            0xFF1C1418, 0xFFFAF1F3, 0xFF2A2025, 0xFF332630,
            0xFF4E2532, 0xFFFFC2D1, 0xFF263A20, 0xFFBFE6A6, 0xFF4A3420, 0xFFFFD3A6,
            0xFF3A2B52, 0xFFD9C8FF, 0xFF1E3940, 0xFFA9DCE8,
            listOf(0xFFC0627E, 0xFF6EA552, 0xFFC08A4E, 0xFF8C6CC4, 0xFF4E95A6),
            listOf(0xFF7E4A58, 0xFF566E48, 0xFF80604A, 0xFF66557E, 0xFF4E6E76, 0xFF7E5650, 0xFF6E6640, 0xFF645860),
        ),
    )

    val SeaGlass = scheme(
        "Sea glass",
        light(
            0xFFEEF5F4, 0xFF122E2C, 0xFF4B6664,
            0xFFC9ECE6, 0xFF14594F, 0xFFD5E6FF, 0xFF1D4F8F, 0xFFF4E6C6, 0xFF6E5214,
            0xFFE2DCF7, 0xFF4A3E8C, 0xFFF6D9D2, 0xFF8A3B2E,
            listOf(0xFF6CC9BA, 0xFF8CB8F2, 0xFFE4C278, 0xFFA99BE0, 0xFFE8A496),
            listOf(0xFF9FCFC6, 0xFFA6C3E8, 0xFFDCC79A, 0xFFB7C9B0, 0xFFC0B5DE, 0xFF93B9C9, 0xFFD6B3A6, 0xFFAFC2C0),
        ),
        dark(
            0xFF0F1A1A, 0xFFECF6F5, 0xFF1B2626, 0xFF22302F,
            0xFF1E403B, 0xFFA8E6DC, 0xFF1E2F4A, 0xFFB5D0FF, 0xFF3D3522, 0xFFF0D898,
            0xFF2C2848, 0xFFCFC6FF, 0xFF452A26, 0xFFFFC2B6,
            listOf(0xFF3E9E8E, 0xFF5A86C8, 0xFFB09348, 0xFF7A6CC0, 0xFFC07264),
            listOf(0xFF3F6E66, 0xFF4A6488, 0xFF7A6A44, 0xFF56704C, 0xFF5E5682, 0xFF3E6476, 0xFF7A5A50, 0xFF4E6664),
        ),
    )

    val Lavender = scheme(
        "Lavender",
        light(
            0xFFF4F2FA, 0xFF221B3A, 0xFF5F5878,
            0xFFE2D8FF, 0xFF4A35A8, 0xFFD2F0E2, 0xFF1E6B48, 0xFFFFDCE5, 0xFF8C2547,
            0xFFFFEBC4, 0xFF7A5200, 0xFFD3E8FA, 0xFF1D5A8A,
            listOf(0xFFA48EF0, 0xFF7ED3AC, 0xFFF59AB2, 0xFFF2C46A, 0xFF8EC0EC),
            listOf(0xFFC3B5EE, 0xFFA9D8C2, 0xFFEFB6C6, 0xFFE6D19C, 0xFFAFC8E6, 0xFFD3B3E0, 0xFFC4CF9E, 0xFFB9B6CC),
        ),
        dark(
            0xFF16131F, 0xFFF3F0FB, 0xFF231F30, 0xFF2A2540,
            0xFF34295C, 0xFFD2C6FF, 0xFF1E3A2E, 0xFFA9E5C8, 0xFF4A2633, 0xFFFFC2D1,
            0xFF453818, 0xFFFFDC8F, 0xFF1E3248, 0xFFAED3FF,
            listOf(0xFF7A62D0, 0xFF4E9C78, 0xFFC0627E, 0xFFC09A3E, 0xFF5D8FC7),
            listOf(0xFF5E4C93, 0xFF4A7060, 0xFF7E4E60, 0xFF7A6A44, 0xFF4C6680, 0xFF6E4E80, 0xFF65703F, 0xFF57536E),
        ),
    )

    val Clay = scheme(
        "Clay",
        light(
            0xFFF5F1EC, 0xFF2A211C, 0xFF6A5D54,
            0xFFF3CFBE, 0xFF8A3A1E, 0xFFDCE4C6, 0xFF4A5A22, 0xFFEFE3CC, 0xFF6E5420,
            0xFFE6DCE8, 0xFF5E3E6E, 0xFFD6E2E4, 0xFF2E5560,
            listOf(0xFFE09A7C, 0xFFAFC07E, 0xFFD8BC84, 0xFFB79AC2, 0xFF8EB2BA),
            listOf(0xFFD9A890, 0xFFBFC9A0, 0xFFDCC7A0, 0xFFC7B0A0, 0xFFB5C2C2, 0xFFD0A8A0, 0xFFB9B08C, 0xFFC2B6AE),
        ),
        dark(
            0xFF1A1613, 0xFFF6F0EA, 0xFF27211D, 0xFF2E2722,
            0xFF4E2C1E, 0xFFF6C3AC, 0xFF2E3520, 0xFFCFDDA6, 0xFF3F3524, 0xFFEFD8A8,
            0xFF3A2A40, 0xFFE6C8F0, 0xFF203236, 0xFFB4D8DE,
            listOf(0xFFB8664A, 0xFF7A8E48, 0xFFA88A50, 0xFF8A6A96, 0xFF5A8890),
            listOf(0xFF7A4E3B, 0xFF5E6844, 0xFF7A6644, 0xFF6E584A, 0xFF4E6466, 0xFF744E48, 0xFF66603E, 0xFF5E544C),
        ),
    )

    val all = listOf(Bento, Sorbet, SeaGlass, Lavender, Clay)

    fun byName(name: String): Scheme = all.firstOrNull { it.name == name } ?: Bento
}
