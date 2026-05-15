use cadrum::{Compound, Edge, Error, Solid};
use glam::DVec3;
fn part(inner: f64, outer: f64, height: f64) -> Result<[Solid; 3], Error> {
	let outer_solid = Solid::cube(outer, outer, height).translate(DVec3::ONE * -outer / 2.0);
	let between_edge = Edge::polygon(&[DVec3::new(outer / 2.0, height - outer / 2.0, 0.0), DVec3::new(outer / 2.0, outer / 2.0, 0.0), DVec3::new(height - outer / 2.0, outer / 2.0, 0.0)])?;
	let between_solid = Solid::extrude(&between_edge, DVec3::Z * outer / 2.0)?;
	let inner_solid = Solid::cylinder(inner / 2.0, DVec3::Z, height * 1000.0);
	Ok([outer_solid, between_solid, inner_solid])
}
fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let (inner, outer, height) = (8.1, 20., 60.0);
	let base = part(inner, outer, height)?;
	let parts = [base.clone(), base.clone().rotate(DVec3::ZERO, DVec3::ONE, std::f64::consts::TAU / 3.0), base.clone().rotate(DVec3::ZERO, DVec3::ONE, std::f64::consts::TAU * 2.0 / 3.0)];
	// positive = 各 part の [outer, between] (= p[0], p[1]) を全部 union
	let positive: Solid = parts.iter().flat_map(|p| [&p[0], &p[1]]).sum::<Result<Solid, _>>()?;
	// negative = 各 part の inner cylinder (= p[2]) を全部 union
	let negative: Solid = parts.iter().map(|p| &p[2]).sum::<Result<Solid, _>>()?;
	let result = [(&positive - &negative)?];

	Solid::write_step(&result, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

	let scene = Solid::mesh(&result, 0.5)?.scene(DVec3::ONE, DVec3::Z, true, true);
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;

	Solid::mesh(&result, 0.1)?.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png / {example_name}.stl");
	Ok(())
}

/*
										   .:.
										::....::.
								   ..::.   ...  .::..
								..:-.   .:-.::-..   .-..
							  .-:.   .-:..  :  ..-.   ..-:.
							 .::.   .::..   :   .::.   .::-
							 ..  ::..   .-..:.::   ..:-.  :
							 ..    .-=..  ....   .:=.     :
							 ..    :. .:::.   .::. .-.    :
							 ..  .-.      .-=:       :.   :
							 ..  -.       .:-:        :.  :
							 ...:.       .-.:.-.       :. :
							 ..-.       .:  : .:        :::
							 .:        .:   :  .-.       .=
							.-.       :.    :   .:.       .-
						   .:        ::     :    .-.       .:
						  .-.      .:.      :      :.       .-.
						 :..       -.       :      .-.       .:.
						.:.      .:.        :        :.       .-
					  .:.       .-.         :        .:.        :.
					  ..       .:           :          ::       .+.
					 -.        -.           :          .::        :.
					:.       .:             :            .:       .-.
				  .-.       .-.             :             .:.       ..
				  :.       .:.              :              .:       .:.
				.-.       .:.               :               .-        .:
			   .:.       ..                 :                .:.       ::
			  .-        .:                 .-                 .+        .-
		   ::.-.       :.               ::....:-.              .:.       .::-.
	   .:-. .-        ::            .::.        .:::.           .-.       .:..::.
   ..::.   .:.      .-.          .-:.               .-..          :.       .:.   .-..
 .-..     ::        :.        .-..                    ..:-.       .-.       .:.    ..::
:::.     .:       .-.      :::.                          ..::.      :.       .-.    .::=
:  .:-..:.       .:.   ..-.                                   ::..   -.        -..:-.  -
:    ...-.      .-. .-:..                                       ..-.  .:       -:..    -
:  .::.  ..::. .-.-:.                                              ..::::  .::.   .-.  -
:  .:..-:.   .:+:.......................................................-=..   .::...  -
:  .:    .-:  ..                                                        .:  .-.    ..  -
:  .:  .::.:  ..                                                        .:  :..:.  ..  -
:  .:::.   :  ..                                                        .:  :.  ..::.  -
:   .::..  :  ..                                                        .:  :.  ..-.   -
.:-.   ..-.:  .............:-..............................:-............:  :.-:..   .-.
   .::.   ..  ..       .-:..                                 .::..      .:  ...  .::.
	   .:..   ..    .:-.                                        .:::.   .:   ..::.
		  .-:... .-:                                                .-: .: .=:.
			..:-..                                                    ..:-..

																							*/
