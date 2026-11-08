async (page) => {
  if (await page.locator('.shacl-validation-diagram, .owl-relationships, table.shacl-properties, .owl-constraint-table').count()) throw new Error('Duplicate class details remain');
  const figure = page.locator('figure.owl-class-diagram').first();
  if (!await figure.locator('svg desc').count()) throw new Error('Missing accessible description');
  if (!await page.getByRole('heading').filter({hasText:'ScheduleShape'}).count()) throw new Error('Dedicated shape documentation missing');
  for (const width of [1280, 390]) {
    await page.setViewportSize({width, height:900});
    if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1)) throw new Error('Document overflow');
    await figure.locator('[role=region]').focus();
  }
  return {duplicateSections:0, dedicatedShape:true, accessibleDescription:true};
}
