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
	let (inner, outer, height) = (8.1, 20., 60.0);
	let base = part(inner, outer, height)?;
	let parts = [base.clone(), base.clone().rotate(DVec3::ZERO, DVec3::ONE, std::f64::consts::TAU / 3.0), base.clone().rotate(DVec3::ZERO, DVec3::ONE, std::f64::consts::TAU * 2.0 / 3.0)];
	let positive = parts.iter().map(|p| Ok::<_, Error>(vec![p[0].clone(), p[1].clone()])).reduce(|a, b| a?.union(&b?)).unwrap()?;
	let negative = parts.iter().map(|p| Ok::<_, Error>(vec![p[2].clone()])).reduce(|a, b| a?.union(&b?)).unwrap()?;
	let result = positive.subtract(&negative)?;
	let mut w = std::fs::File::create("joints.step").unwrap();
	Solid::write_step(&result, &mut w).unwrap();
	let mut w = std::fs::File::create("joints.stl").unwrap();
	Solid::mesh(&result, 0.1).unwrap().write_stl(&mut w).unwrap();
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
